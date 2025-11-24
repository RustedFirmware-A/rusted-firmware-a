// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! PSCI OSI tests for FVP platform.

use crate::{
    framework::{
        TestError, TestResult,
        expect::{expect_eq, fail},
        normal_world_test,
    },
    gicv3::set_interrupt_handler,
    platform::{Platform, PlatformImpl},
    start_secondary,
    tests::psci::is_osi_supported,
    util::log_error,
    util::timer::{NonSecureTimer, Timer},
};
use aarch64_rt::{enable_mmu, set_exception_vector};
use arm_gic::{Trigger, irq_disable, irq_enable};
use arm_psci::{ErrorCode, FunctionId, ReturnCode};
use arm_sysregs::read_mpidr_el1;
use core::{
    arch::naked_asm,
    hint::spin_loop,
    sync::atomic::{AtomicBool, AtomicU32, Ordering},
};
use log::trace;
use smccc::{
    Smc,
    psci::{self, AffinityState, LowestAffinityLevel},
};

fn with_osi_support(f: impl FnOnce() -> TestResult) -> TestResult {
    if !is_osi_supported()? {
        return Err(TestError::Ignored);
    }

    expect_eq!(
        psci::set_suspend_mode::<Smc>(psci::SuspendMode::OsInitiated),
        Ok(())
    );

    let result = f();

    expect_eq!(
        psci::set_suspend_mode::<Smc>(psci::SuspendMode::PlatformCoordinated),
        Ok(())
    );

    result
}

/// Polls `AFFINITY_INFO` to verify a core has fully powered down.
fn wait_for_core_off(mpidr: u64) -> TestResult {
    loop {
        let info = log_error(
            "AffinityInfo failed",
            psci::affinity_info::<Smc>(mpidr, LowestAffinityLevel::All),
        )?;
        if info == AffinityState::Off {
            break;
        }
        spin_loop();
    }
    Ok(())
}

/// Wrapper for `CPU_SUSPEND` SMC that saves/restores callee-saved registers and the timer context.
///
/// On resume, it restores the stack pointer from the `context_id` (returned in x0),
/// re-enables the MMU, and restores exception vectors.
#[unsafe(naked)]
unsafe extern "C" fn cpu_suspend_save_context(function_id: u32, power_state: u32) -> i32 {
    naked_asm!(
        // Argument x0: function_id
        // Argument x1: power_state

        // Save callee-saved registers to the stack.
        "stp x19, x20, [sp, #-16]!",
        "stp x21, x22, [sp, #-16]!",
        "stp x23, x24, [sp, #-16]!",
        "stp x25, x26, [sp, #-16]!",
        "stp x27, x28, [sp, #-16]!",
        "stp x29, x30, [sp, #-16]!",

        // Save timer registers
        "mrs x19, cntp_cval_el0",
        "mrs x20, cntp_ctl_el0",
        "stp x19, x20, [sp, #-16]!",

        // Arguments for the SMC call:
        // x0: Function ID (already in x0)
        // x1: Power State (already in x1)
        // x2: Entry Point
        "adr x2, 1f",
        // x3: Context ID
        "mov x3, sp",

        "smc #0",

        // Fallthrough on failure or standby (context preserved).
        // Pop timer registers and jump to restore.
        "add sp, sp, #16",
        "b 2f",

        "1:", // Resume entry point. x0 contains context_id (saved SP).
        // Restore the stack pointer.
        "mov sp, x0",

        // Re-enable the MMU.
        "bl {enable_mmu}",
        // Restore the exception vector table.
        "bl {set_exception_vector}",

        // Restore timer registers
        "ldp x19, x20, [sp], #16",
        "msr cntp_cval_el0, x19",
        "msr cntp_ctl_el0, x20",

        // Set return value to 0 (Success) for the Rust caller.
        "mov x0, #0",

        "2:",
        // Restore callee-saved registers from the stack.
        "ldp x29, x30, [sp], #16",
        "ldp x27, x28, [sp], #16",
        "ldp x25, x26, [sp], #16",
        "ldp x23, x24, [sp], #16",
        "ldp x21, x22, [sp], #16",
        "ldp x19, x20, [sp], #16",
        "ret",
        enable_mmu = sym enable_mmu,
        set_exception_vector = sym set_exception_vector,
    );
}

/// Synchronization primitives for OSI test coordination.
struct OsiCoreState {
    state_id: AtomicU32,
    last_level: AtomicU32,
    booted: AtomicBool,
    ready: AtomicBool,
    done: AtomicBool,
    irq_received: AtomicBool,
    duration: AtomicU32,
    should_suspend: AtomicBool,
}

impl OsiCoreState {
    const fn new() -> Self {
        Self {
            state_id: AtomicU32::new(0),
            last_level: AtomicU32::new(0),
            booted: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            done: AtomicBool::new(false),
            irq_received: AtomicBool::new(false),
            duration: AtomicU32::new(0),
            should_suspend: AtomicBool::new(true),
        }
    }
}

static OSI_CORES: [OsiCoreState; PlatformImpl::CORE_COUNT] =
    [const { OsiCoreState::new() }; PlatformImpl::CORE_COUNT];

/// Interrupt handler for the non-secure timer. Signals completion of the suspend duration.
fn timer_handler() {
    NonSecureTimer::stop();
    let core_idx = PlatformImpl::core_position(read_mpidr_el1());
    OSI_CORES[core_idx]
        .irq_received
        .store(true, Ordering::SeqCst);
}

fn set_timer_handler() {
    set_interrupt_handler(
        NonSecureTimer::INTERRUPT_ID,
        Trigger::Level,
        Some(timer_handler),
    );
}

fn teardown_timer() {
    NonSecureTimer::stop();
    set_interrupt_handler(NonSecureTimer::INTERRUPT_ID, Trigger::Level, None);
}

/// Executes `CPU_SUSPEND` with the specified state.
fn suspend_and_resume(core_idx: usize, retry_on_denied: bool) -> i32 {
    // Construct the power state parameter for CPU_SUSPEND.
    let state_id = OSI_CORES[core_idx].state_id.load(Ordering::SeqCst);
    let pstate = PlatformImpl::make_osi_power_state(
        state_id,
        OSI_CORES[core_idx].last_level.load(Ordering::SeqCst),
    );

    // Disable IRQs before programming the timer.
    irq_disable();

    set_timer_handler();
    NonSecureTimer::set(OSI_CORES[core_idx].duration.load(Ordering::SeqCst));

    trace!("suspend_and_resume[{core_idx}]: pstate = {pstate:#x}");

    let mut result;
    loop {
        // SAFETY: Calling the SMC wrapper is safe.
        result = unsafe { cpu_suspend_save_context(u32::from(FunctionId::CpuSuspend64), pstate) };
        if !retry_on_denied || result != ErrorCode::Denied as i32 {
            break;
        }
        // If retry is enabled (e.g. for primary core waiting for secondaries),
        // spin and try again.
        spin_loop();
    }

    trace!("suspend_and_resume[{core_idx}]: pstate = {pstate:#x} -> {result}");

    if result == 0 && state_id != PlatformImpl::osi_state_id_core_standby() {
        // Restore GIC interface after power down.
        crate::gicv3::init_core();
    } else {
        // Re-enable IRQs.
        irq_enable();
    }

    if result == 0 {
        // Wait for the timer interrupt to be handled.
        while !OSI_CORES[core_idx].irq_received.load(Ordering::SeqCst) {
            spin_loop();
        }
    }

    result
}

/// Entry point for secondary cores during OSI tests.
///
/// Synchronizes with the primary core, optionally suspends, signals completion, and powers down.
fn osi_secondary_entry(arg: u64) -> ! {
    let core_idx = arg as usize;

    // Signal that this core has booted and is running.
    OSI_CORES[core_idx].booted.store(true, Ordering::SeqCst);

    // Wait for the primary core to signal readiness to proceed.
    while !OSI_CORES[core_idx].ready.load(Ordering::SeqCst) {
        spin_loop();
    }

    if OSI_CORES[core_idx].should_suspend.load(Ordering::SeqCst) {
        // Perform the suspend operation.
        suspend_and_resume(core_idx, false);
    } else {
        // Just stay awake and wait for the test to complete.
        // We still need to wait for the main test logic to tell us to finish.
        // The `done` signal below tells the main thread we have reached the synchronization point.
    }

    OSI_CORES[core_idx].done.store(true, Ordering::SeqCst);

    // Wait for the test coordination to signal teardown (Ready cleared).
    while OSI_CORES[core_idx].ready.load(Ordering::SeqCst) {
        spin_loop();
    }

    psci::cpu_off::<Smc>().unwrap();
    loop {
        spin_loop();
    }
}

/// Orchestrates an OSI suspend test:
///
/// 1. Resets shared state.
/// 2. Configures target states and 'Last Man' expectations for all cores.
/// 3. Boots secondary cores.
/// 4. Executes `CPU_SUSPEND` on the primary core.
/// 5. Verifies the result and ensures all cores teardown cleanly.
fn run_osi_suspend_test(
    target_aff_level: usize,
    target_state_id: u32,
    core_to_keep_awake: Option<usize>,
    expected_result: ReturnCode,
) -> TestResult {
    let is_standby = target_state_id == PlatformImpl::osi_state_id_core_standby();
    let topology = PlatformImpl::osi_test_topology();

    // Initialize synchronization primitives and test parameters.
    for (i, core) in OSI_CORES.iter().enumerate().take(PlatformImpl::CORE_COUNT) {
        core.booted.store(false, Ordering::SeqCst);
        core.ready.store(false, Ordering::SeqCst);
        core.done.store(false, Ordering::SeqCst);
        core.irq_received.store(false, Ordering::SeqCst);

        // Default target state for all cores.
        core.state_id.store(target_state_id, Ordering::SeqCst);
        // Default last level = 0 (CPU level).
        core.last_level.store(0, Ordering::SeqCst);
        // Default duration approx 10ms.
        core.duration
            .store(PlatformImpl::osi_suspend_duration_ticks(), Ordering::SeqCst);

        let should_suspend = core_to_keep_awake != Some(i);
        core.should_suspend.store(should_suspend, Ordering::SeqCst);
    }

    // Configure last level and coordination for cluster-level tests.
    if target_aff_level > 0 {
        // Deep sleep (cluster+) requires coordination.
        // Core 0 is "last man". Others suspend to target level but claim last man
        // only at level 0.

        // Core 0: Last man at the target affinity level.
        OSI_CORES[0]
            .last_level
            .store(target_aff_level as u32, Ordering::SeqCst);

        // Iterate over clusters to configure secondary cores.
        let mut base_idx = 0;
        for (cluster_idx, &cluster_size) in topology.iter().enumerate() {
            let cluster_end = base_idx + cluster_size - 1;

            for (i, core) in OSI_CORES
                .iter()
                .enumerate()
                .take(cluster_end + 1)
                .skip(base_idx)
            {
                // Skip lead CPU.
                if i == 0 {
                    continue;
                }

                if core.should_suspend.load(Ordering::SeqCst) {
                    // Non-lead cores should just power down/standby themselves (Level 0).
                    // They should NOT request the target level (e.g. cluster) because they are not
                    // the last man.
                    //
                    // Exception: If target is level 2 (system), the last man of the last cluster
                    // must request level 1 (cluster off) to allow the system to turn off.
                    let mut local_state = PlatformImpl::osi_state_id_core_power_down();
                    let mut local_lvl = 0;

                    if is_standby {
                        local_state = PlatformImpl::osi_state_id_core_standby();
                    } else if target_aff_level == 2 && i == cluster_end && cluster_idx > 0 {
                        // The last man of a non-lead cluster must request cluster power down
                        // to allow system suspend.
                        local_state = PlatformImpl::osi_state_id_cluster_power_down();
                        local_lvl = 1;
                    }

                    core.state_id.store(local_state, Ordering::SeqCst);
                    core.last_level.store(local_lvl, Ordering::SeqCst);
                    // Sleep longer to allow Primary to check status and suspend.
                    core.duration.store(
                        PlatformImpl::osi_suspend_duration_ticks()
                            * PlatformImpl::CORE_COUNT as u32,
                        Ordering::SeqCst,
                    );
                }
            }
            base_idx += cluster_size;
        }
    }

    // Power on secondary cores.
    for i in 1..PlatformImpl::CORE_COUNT {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        if start_secondary(
            PlatformImpl::psci_mpidr_for_core(i),
            osi_secondary_entry,
            i as u64,
        )
        .is_err()
        {
            fail!("Failed to start core {}", i);
        }
    }

    // Wait for each core to enter the test.
    for (i, core) in OSI_CORES
        .iter()
        .enumerate()
        .take(PlatformImpl::CORE_COUNT)
        .skip(1)
    {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        while !core.booted.load(Ordering::SeqCst) {
            spin_loop();
        }
    }

    // Signal each secondary core to suspend itself (if requested).
    for (i, core) in OSI_CORES
        .iter()
        .enumerate()
        .take(PlatformImpl::CORE_COUNT)
        .skip(1)
    {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        core.ready.store(true, Ordering::SeqCst);
        NonSecureTimer::delay_us(PlatformImpl::osi_suspend_entry_delay_us());
    }

    // Suspend the primary core.
    let retry = expected_result == ReturnCode::Success;
    let result = suspend_and_resume(0, retry);

    let result = ReturnCode::try_from(result).unwrap();

    trace!(
        "run_osi_suspend_test: target_aff_level = {target_aff_level}, target_state_id = {target_state_id:#x}, awake = {core_to_keep_awake:?} -> {result:?}"
    );

    expect_eq!(result, expected_result);

    // Wait for secondary cores to wake up.
    for (i, core) in OSI_CORES
        .iter()
        .enumerate()
        .take(PlatformImpl::CORE_COUNT)
        .skip(1)
    {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        while !core.done.load(Ordering::SeqCst) {
            spin_loop();
        }
    }

    teardown_timer();

    // Signal secondary core teardown.
    for (i, core) in OSI_CORES
        .iter()
        .enumerate()
        .take(PlatformImpl::CORE_COUNT)
        .skip(1)
    {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        core.ready.store(false, Ordering::SeqCst);
    }

    // Wait for secondary cores to power down.
    for i in 1..PlatformImpl::CORE_COUNT {
        if !PlatformImpl::osi_should_wake_core(i) {
            continue;
        }
        wait_for_core_off(PlatformImpl::psci_mpidr_for_core(i))?;
    }

    Ok(())
}

normal_world_test!(test_psci_suspend_powerdown_level0_osi);
fn test_psci_suspend_powerdown_level0_osi() -> TestResult {
    with_osi_support(|| {
        // Core 0 powers down at level 0 (core). No coordination required.
        run_osi_suspend_test(
            0,
            PlatformImpl::osi_state_id_core_power_down(),
            None,
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_standby_level0_osi);
fn test_psci_suspend_standby_level0_osi() -> TestResult {
    with_osi_support(|| {
        // Core 0 enters standby at level 0 (core).
        run_osi_suspend_test(
            0,
            PlatformImpl::osi_state_id_core_standby(),
            None,
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_powerdown_level1_osi);
fn test_psci_suspend_powerdown_level1_osi() -> TestResult {
    with_osi_support(|| {
        // Core 0 powers down at level 1 (cluster). All other cores in the cluster also suspend.
        run_osi_suspend_test(
            1,
            PlatformImpl::osi_state_id_cluster_power_down(),
            None,
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_standby_level1_osi);
fn test_psci_suspend_standby_level1_osi() -> TestResult {
    with_osi_support(|| {
        // Core 0 enters standby at level 1 (cluster). All other cores in the cluster also suspend.
        run_osi_suspend_test(
            1,
            PlatformImpl::osi_state_id_core_standby(),
            None,
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_powerdown_level2_osi);
fn test_psci_suspend_powerdown_level2_osi() -> TestResult {
    with_osi_support(|| {
        // Core 0 powers down at level 2 (system).
        // Requires other clusters to also power down.
        run_osi_suspend_test(
            2,
            PlatformImpl::osi_state_id_system_power_down(),
            None,
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_osi_denied_same_cluster_on);
fn test_psci_suspend_osi_denied_same_cluster_on() -> TestResult {
    with_osi_support(|| {
        let topology = PlatformImpl::osi_test_topology();
        if topology[0] < 2 {
            return Ok(());
        }
        // Core 0 requests cluster (level 1) power down, but a sibling core remains on. Expect Denied.
        run_osi_suspend_test(
            1,
            PlatformImpl::osi_state_id_cluster_power_down(),
            Some(1),
            ReturnCode::Error(ErrorCode::Denied),
        )
    })
}

normal_world_test!(test_psci_suspend_osi_denied_level0_with_level1_claim);
fn test_psci_suspend_osi_denied_level0_with_level1_claim() -> TestResult {
    with_osi_support(|| {
        let topology = PlatformImpl::osi_test_topology();
        if topology[0] < 2 {
            return Ok(());
        }
        // Core 0 requests core (level 0) power down but claims last man at cluster (level 1),
        // while a sibling core is on. Expect Denied.
        run_osi_suspend_test(
            1,
            PlatformImpl::osi_state_id_core_power_down(),
            Some(1),
            ReturnCode::Error(ErrorCode::Denied),
        )
    })
}

normal_world_test!(test_psci_suspend_osi_cluster0_off_other_cluster_on);
fn test_psci_suspend_osi_cluster0_off_other_cluster_on() -> TestResult {
    with_osi_support(|| {
        let topology = PlatformImpl::osi_test_topology();
        if topology.len() < 2 {
            return Ok(());
        }
        let other_cluster_core = topology[0]; // First core of second cluster.

        // Core 0 requests cluster 0 (level 1) power down. Core in cluster 1 remains on.
        // Expect Success as clusters power down independently.
        run_osi_suspend_test(
            1,
            PlatformImpl::osi_state_id_cluster_power_down(),
            Some(other_cluster_core),
            ReturnCode::Success,
        )
    })
}

normal_world_test!(test_psci_suspend_osi_denied_system_same_cluster_on);
fn test_psci_suspend_osi_denied_system_same_cluster_on() -> TestResult {
    with_osi_support(|| {
        let topology = PlatformImpl::osi_test_topology();
        if topology[0] < 2 {
            return Ok(());
        }

        // Core 0 requests system (level 2) power down. A sibling core (same cluster) is on.
        // Expect Denied.
        run_osi_suspend_test(
            2,
            PlatformImpl::osi_state_id_system_power_down(),
            Some(1),
            ReturnCode::Error(ErrorCode::Denied),
        )
    })
}

normal_world_test!(test_psci_suspend_osi_denied_system_other_cluster_on);
fn test_psci_suspend_osi_denied_system_other_cluster_on() -> TestResult {
    with_osi_support(|| {
        let topology = PlatformImpl::osi_test_topology();
        if topology.len() < 2 {
            return Ok(());
        }
        let other_cluster_core = topology[0]; // First core of second cluster.

        // Core 0 requests system (level 2) power down. Core in other cluster is on.
        // Expect Denied.
        run_osi_suspend_test(
            2,
            PlatformImpl::osi_state_id_system_power_down(),
            Some(other_cluster_core),
            ReturnCode::Error(ErrorCode::Denied),
        )
    })
}

normal_world_test!(test_psci_suspend_osi_invalid_level);
fn test_psci_suspend_osi_invalid_level() -> TestResult {
    with_osi_support(|| {
        for &pstate in PlatformImpl::osi_invalid_power_states() {
            // SAFETY: It's safe to call the SMC wrapper in this context.
            let ret =
                unsafe { cpu_suspend_save_context(u32::from(FunctionId::CpuSuspend64), pstate) };
            expect_eq!(ret, ErrorCode::InvalidParameters as i32);
        }
        Ok(())
    })
}
