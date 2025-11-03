// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

mod power_domain_tree;

#[cfg(not(test))]
use crate::services::Services;
#[cfg(test)]
use crate::services::ffa::TestSpm;
use crate::{
    aarch64::{dsb_sy, wfi},
    context::{CoresImpl, World},
    cpu::cpu_power_down,
    platform::{Platform, PlatformImpl, PlatformPowerState, PsciPlatformImpl},
    services::{Service, owns},
    smccc::{FunctionId as SmcFunctionId, OwningEntityNumber, SetFrom, SmcReturn},
};
use arm_psci::{
    AffinityInfo, Cookie, EntryPoint, ErrorCode, FeatureFlagsCpuSuspend, FeatureFlagsSystemOff2,
    Function, FunctionId, HwState, MemProtectRange, MigrateInfoType, Mpidr, PowerState,
    PsciFeature, ResetType, ReturnCode, SuspendMode, SystemOff2Type, Version,
};
use arm_sysregs::{MpidrEl1, read_isr_el1};
use bitflags::bitflags;
use core::fmt::{self, Debug, Formatter};
use log::info;
use percore::Cores;
use power_domain_tree::{AncestorPowerDomains, CpuPowerNode, PowerDomainTree};
use spin::mutex::SpinMutex;

const FUNCTION_NUMBER_MIN: u16 = 0x0000;
const FUNCTION_NUMBER_MAX: u16 = 0x001F;

bitflags! {
    /// Optional platform feature flags
    #[derive(Debug, Eq, PartialEq, Clone, Copy)]
    #[repr(transparent)]
    pub struct PsciPlatformOptionalFeatures: u64 {
        const SYSTEM_OFF2 = 1 << 0;
        const SYSTEM_RESET2 = 1 << 1;
        const MEM_PROTECT = 1 << 2;
        const MEM_PROTECT_CHECK_RANGE = 1 << 3;
        const CPU_FREEZE = 1 << 4;
        const CPU_DEFAULT_SUSPEND = 1 << 5;
        const NODE_HW_STATE = 1 << 6;
        const SYSTEM_SUSPEND = 1 << 7;
        const OS_INITIATED_MODE = 1 << 8;
    }
}

/// Platform-specific power state interface
///
/// The platform has to provide a platform-specific power state type which implements this trait
/// and all of the dependent traits.
///
/// The type has to implement the `Ord` trait in a way the states are in ascending order from
/// running state to power down state.
pub trait PlatformPowerStateInterface:
    Debug + Clone + Copy + PartialEq + Ord + Into<usize>
{
    const OFF: Self;
    const RUN: Self;

    /// Returns the type of the platform-specific power state.
    fn power_state_type(&self) -> PowerStateType;
}

/// PSCI platform interface
///
/// The interface contains mandatory and optional constants and functions. Whether the platform
/// implements the optional functions has to be in sync with the reported optional features in the
/// `FEATURES` constant.
pub trait PsciPlatformInterface {
    /// Count of all power domains
    const POWER_DOMAIN_COUNT: usize;
    /// Maximal power level in the system
    const MAX_POWER_LEVEL: usize;

    /// Flags for describing optional features implemented by the platform.
    const FEATURES: PsciPlatformOptionalFeatures;

    /// Platform-specific power state type
    type PlatformPowerState: PlatformPowerStateInterface;

    /// Returns the power domain topology as the count of child nodes in a BFS traversal order.
    fn topology() -> &'static [usize];

    /// Tries to convert extended PSCI power state value into `PsciCompositePowerState`.
    fn try_parse_power_state(power_state: PowerState) -> Option<PsciCompositePowerState>;

    /// Places the current CPU into standby state and continues execution on interrupt.
    /// The caller has to guarantee that `cpu_state` is a standby power state, otherwise
    /// `cpu_standby` should panic.
    fn cpu_standby(&self, cpu_state: PlatformPowerState);

    /// Performs the necessary actions to turn off this cpu e.g. program the power controller.
    fn power_domain_suspend(&self, target_state: &PsciCompositePowerState);

    /// Performs platform-specific operations after a wake-up from standby/retention states.
    fn power_domain_suspend_finish(&self, previous_state: &PsciCompositePowerState);

    /// Callback for platform housekeeping before turning off the CPU, optional.
    fn power_domain_off_early(
        &self,
        _target_state: &PsciCompositePowerState,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }

    /// Perform platform-specific actions to turn this cpu off e.g. program the power controller.
    fn power_domain_off(&self, target_state: &PsciCompositePowerState);

    /// Platform-specific function for entering WFI on power down, optional.
    fn power_domain_power_down_wfi(&self, _target_state: &PsciCompositePowerState) -> ! {
        dsb_sy();
        loop {
            wfi();
        }
    }

    /// Turn on power domain, which is identified by its MPIDR.
    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode>;

    /// Perform platform-specific actions after the CPU has been turned on.
    fn power_domain_on_finish(&self, previous_state: &PsciCompositePowerState);

    /// Shuts down the system.
    fn system_off(&self) -> !;

    /// Suspend to disk, optional.
    fn system_off2(&self, _off_type: SystemOff2Type, _cookie: Cookie) -> Result<(), ErrorCode> {
        unimplemented!("SYSTEM_OFF2 is not implemented for the platform")
    }

    /// Resets the system, the behavior is equivalent to a hardware power-cycle sequence.
    fn system_reset(&self) -> !;

    /// Architectural or vendor specific reset function, optional.
    fn system_reset2(&self, _reset_type: ResetType, _cookie: Cookie) -> Result<(), ErrorCode> {
        unimplemented!("SYSTEM_RESET2 is not implemented for the platform")
    }

    /// Enable memory protection and returns the previous state, optional.
    fn mem_protect(&self, _enabled: bool) -> Result<bool, ErrorCode> {
        unimplemented!("MEM_PROTECT is not implemented for the platform")
    }

    /// Checks if the memory range is protected, optional.
    fn mem_protect_check_range(&self, _range: MemProtectRange) -> Result<(), ErrorCode> {
        unimplemented!("MEM_PROTECT_CHECK_RANGE is not implemented for the platform")
    }

    /// Places a core into an implementation defined low-power state where an interrupt does not
    /// return the core back into a running state.
    fn cpu_freeze(&self) -> ! {
        unimplemented!("CPU_FREEZE is not implemented for the platform")
    }

    /// Returns the power state for `CPU_DEFAULT_SUSPEND`, optional.
    fn cpu_default_suspend_power_state(&self) -> PowerState {
        unimplemented!("CPU_DEFAULT_SUSPEND is not implemented for the platform")
    }

    /// Returns the true hardware state of a power domain, optional.
    fn node_hw_state(&self, _mpidr: Mpidr, _power_level: u32) -> Result<HwState, ErrorCode> {
        unimplemented!("NODE_HW_STATE is not implemented for the platform")
    }

    /// Returns the power state for `SYSTEM_SUSPEND`, optional.
    fn sys_suspend_power_state(&self) -> PsciCompositePowerState {
        unimplemented!("SYSTEM_SUSPEND is not implemented for the platform")
    }

    /// Validates a non-secure entry point, optional.
    fn is_valid_ns_entrypoint(&self, _entry: &EntryPoint) -> bool {
        true
    }

    /// Checks if the CPU has pending interrupts
    fn has_pending_interrupts(&self) -> bool {
        read_isr_el1() != 0
    }
}

/// PSCI SPM interface.
///
/// Contains the callbacks that the PSCI implementation uses to inform the Secure World about power
/// management events.
pub trait PsciSpmInterface {
    /// Forward a PSCI request to the SPM.
    ///
    /// The request should be forwarded by the SPMD to the SPMC if it resides in a separate
    /// exception level, or forwarded by the SPMC in EL3 to the SPs.
    fn forward_psci_request(&self, psci_request: &[u64; 4]) -> u64;

    /// Notify the SPM about a CPU_OFF event.
    ///
    /// The PSCI service has received a CPU_OFF request, so the current core will be turned off.
    /// Before calling this function, the PSCI request itself should be forwarded to SWd using
    /// forward_psci_request()
    fn notify_cpu_off(&self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerStateType {
    PowerDown,
    StandbyOrRetention,
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeUpReason {
    CpuOn(EntryPoint),
    SuspendFinished(EntryPoint),
}

/// Object for storing platform-specific power state for multiple power levels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsciCompositePowerState {
    pub states: [PlatformPowerState; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
}

impl PsciCompositePowerState {
    pub const CPU_POWER_LEVEL: usize = 0;

    /// States set to OFF on all levels.
    pub const OFF: Self = Self {
        states: [PlatformPowerState::OFF; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
    };

    /// States set to RUN on all levels.
    pub const RUN: Self = Self {
        states: [PlatformPowerState::RUN; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
    };

    #[allow(unused)]
    pub fn new(states: [PlatformPowerState; PsciPlatformImpl::MAX_POWER_LEVEL + 1]) -> Self {
        Self { states }
    }

    /// Returns the power state of the CPU level.
    pub fn cpu_level_state(&self) -> PlatformPowerState {
        self.states[Self::CPU_POWER_LEVEL]
    }

    /// Returns the power state of the highest level of the topology.
    pub fn highest_level_state(&self) -> PlatformPowerState {
        self.states[PsciPlatformImpl::MAX_POWER_LEVEL]
    }

    /// Find the highest power level which is not set to running state.
    pub fn find_highest_non_run_level(&self) -> Option<usize> {
        self.states
            .iter()
            .rposition(|state| state.power_state_type() != PowerStateType::Run)
    }

    /// Find the highest power level which is set to power down state.
    pub fn find_highest_power_down_level(&self) -> Option<usize> {
        self.states
            .iter()
            .rposition(|state| state.power_state_type() == PowerStateType::PowerDown)
    }

    /// Fill the structure with the current local states of the given CPU node and its ancestor
    /// non-CPU power domain nodes.
    pub fn set_local_states_from_nodes(
        &mut self,
        cpu: &CpuPowerNode,
        ancestors: &AncestorPowerDomains,
    ) {
        self.states[PsciCompositePowerState::CPU_POWER_LEVEL] = cpu.local_state();

        for (node, state) in ancestors
            .iter()
            .zip(&mut self.states[PsciCompositePowerState::CPU_POWER_LEVEL + 1..])
        {
            *state = node.local_state();
        }
    }

    /// Requests the power state for all ancestor nodes and sets the minimal local state for each
    /// node. When a CPU node enters a lower power state, its ancestor nodes may also be able to
    /// transition to a lower power state. Each non-CPU power node maintains a list of power states
    /// requested by its descendant nodes. This function sets the lowest power state permitted by
    /// the list of requested states.
    pub fn coordinate_state(&mut self, cpu_index: usize, ancestors: &mut AncestorPowerDomains) {
        let mut higher_levels_are_run = false;

        for (node, state) in ancestors
            .iter_mut()
            .zip(&mut self.states[PsciCompositePowerState::CPU_POWER_LEVEL + 1..])
        {
            node.set_requested_power_state(cpu_index, *state);

            if !higher_levels_are_run {
                node.set_minimal_allowed_state();
                *state = node.local_state();

                if state.power_state_type() == PowerStateType::Run {
                    // We reached a level where running states is required, so all power states
                    // on the higher level can be set to run.
                    higher_levels_are_run = true;
                }
            } else {
                // If there was a running state on a previous level, there's no need for finding
                // the minimal allowed state because it can only be in running state.
                *state = PlatformPowerState::RUN;
            }
        }
    }

    /// Checks that the composite state does not violate any PSCI rules.
    pub fn is_valid_suspend_request(&self, is_power_down_state: bool) -> bool {
        // There should be a non-run level
        if self.find_highest_non_run_level().is_none() {
            return false;
        };

        // Higher levels must be in less than or equal power state
        if !self.states.is_sorted_by(|a, b| a >= b) {
            return false;
        }

        if is_power_down_state {
            // There must be a power down state
            self.find_highest_power_down_level().is_some()
        } else {
            // Retention state, there should not be a power state on any level
            self.find_highest_power_down_level().is_none()
        }
    }
}

/// Main PSCI structure of the PSCI implementation that handles all the PSCI calls and stores the
/// the power state representation of each power domain.
pub struct Psci {
    platform: PsciPlatformImpl,
    power_domain_tree: PowerDomainTree,
    suspend_mode: SpinMutex<SuspendMode>,
}

impl Psci {
    /// Initialises the PSCI state.
    ///
    /// This should be called exactly once, before any other PSCI methods are called or any
    /// secondary CPUs are started.
    pub(super) fn new(platform: PsciPlatformImpl) -> Self {
        info!("Initializing PSCI");

        let power_domain_tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        {
            // Init primary CPU
            let cpu_index = CoresImpl::core_index();
            let mut cpu = power_domain_tree.locked_cpu_node(cpu_index);

            power_domain_tree.with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                cpu.set_affinity_info(AffinityInfo::On);
                cpu.set_local_state(PlatformPowerState::RUN);

                for node in ancestors.iter_mut() {
                    node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                    node.set_local_state(PlatformPowerState::RUN);
                }
            });
        }
        let suspend_mode = SpinMutex::<_>::new(SuspendMode::PlatformCoordinated);

        Self {
            platform,
            power_domain_tree,
            suspend_mode,
        }
    }

    /// Handles `CPU_SUSPEND` PSCI call by following the steps below.
    /// * If the a standby power state is requested which only affects the CPU level, the wait for
    ///   interrupts by calling `cpu_standby` and then return after an interrupt.
    /// * If a power down state is requested or a standby request affects higher levels, then call
    ///   `cpu_suspend_start`.
    fn cpu_suspend(
        &self,
        power_state: PowerState,
        entry_point: EntryPoint,
    ) -> Result<(), ErrorCode> {
        let cpu_index = CoresImpl::core_index();
        let composite_state: PsciCompositePowerState =
            PsciPlatformImpl::try_parse_power_state(power_state)
                .ok_or(ErrorCode::InvalidParameters)?;

        let is_power_down_state = matches!(power_state, PowerState::PowerDown(_));

        assert!(composite_state.is_valid_suspend_request(is_power_down_state));

        let highest_affected_level = composite_state
            .find_highest_non_run_level()
            .expect("Invalid target power level for suspend operation");

        if !is_power_down_state
            && highest_affected_level == PsciCompositePowerState::CPU_POWER_LEVEL
        {
            // CPU standby which does not affect parent nodes
            let cpu_pd_state = composite_state.cpu_level_state();
            self.power_domain_tree
                .locked_cpu_node(cpu_index)
                .set_local_state(cpu_pd_state);

            // Start waiting for interrupts.
            self.platform.cpu_standby(cpu_pd_state);
            // Continue execution after an interrupt woke up the CPU.

            self.power_domain_tree
                .locked_cpu_node(cpu_index)
                .set_local_state(PlatformPowerState::RUN);

            Ok(())
        } else {
            if is_power_down_state && !self.platform.is_valid_ns_entrypoint(&entry_point) {
                return Err(ErrorCode::InvalidAddress);
            }

            self.cpu_suspend_start(
                cpu_index,
                Some(power_state),
                entry_point,
                highest_affected_level,
                composite_state,
                is_power_down_state,
            )
        }
    }

    /// Handles the common part of `CPU_SUSPEND` and `SYSTEM_SUSPEND` PSCI calls. The `power_state`
    /// argument is `None` when coming from `SYSTEM_SUSPEND` handler because it does not have
    /// power state parameter.
    ///
    /// The function follows the steps below.
    /// * Return immediately if there's a pending interrupt.
    /// * Otherwise determine the valid state for each level without violating any power domain
    ///   rules.
    /// * Request this power state from the platform layer (`power_domain_suspend`). This step does
    ///   not trigger an immediate shutdown of the power domain.
    /// * Power down the domain by calling `power_domain_power_down_wfi` if this is a power down
    ///   request. Normally this function calls `WFI` in a loop which actually turns off the domain
    ///   but platforms may diverge from this. `cpu_suspend_start` does not return after this point.
    ///   When the CPU wakes up, the boot code must call `handle_cpu_boot` that completes the power
    ///   down suspend operation.
    /// * If the requested power state is a standby state, call a `WFI` and restore running state
    ///   after waking up by an interrupt.
    fn cpu_suspend_start(
        &self,
        cpu_index: usize,
        power_state: Option<PowerState>,
        entry: EntryPoint,
        highest_affected_level: usize,
        mut composite_state: PsciCompositePowerState,
        is_power_down_state: bool,
    ) -> Result<(), ErrorCode> {
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let has_pending_interrupt = self.power_domain_tree.with_ancestors_locked_to_max_level(
            &mut cpu,
            highest_affected_level,
            |cpu, mut ancestors| {
                if self.platform.has_pending_interrupts() {
                    return true;
                }

                composite_state.coordinate_state(cpu_index, &mut ancestors);
                cpu.set_local_state(composite_state.cpu_level_state());

                if is_power_down_state {
                    if let Some(state) = power_state {
                        self.forward_to_spm(Function::CpuSuspend { state, entry });
                    } else {
                        self.forward_to_spm(Function::SystemSuspend { entry });
                    }
                    cpu.set_entry_point(entry);

                    cpu_power_down(composite_state.find_highest_power_down_level().unwrap());
                }

                self.platform.power_domain_suspend(&composite_state);
                false
            },
        );
        drop(cpu); // Unlock CPU before entering suspend state

        if has_pending_interrupt {
            // Has pending interrupts, do not suspend
            return Ok(());
        }

        // Continue suspend operation
        if is_power_down_state {
            self.platform.power_domain_power_down_wfi(&composite_state);
            // This branch does not return
        } else {
            // Go to suspend by waiting for interrupts.
            wfi();

            // Restore running state after wake-up.
            let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
            self.power_domain_tree
                .with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                    composite_state.set_local_states_from_nodes(cpu, &ancestors);

                    self.platform.power_domain_suspend_finish(&composite_state);
                    cpu.clear_highest_affected_level();
                    cpu.set_local_state(PlatformPowerState::RUN);

                    for node in ancestors.iter_mut() {
                        node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                        node.set_local_state(PlatformPowerState::RUN);
                    }
                });
            Ok(())
        }
    }

    /// Handles `CPU_OFF` PSCI call.
    /// On success, turns off the current CPU and does not return.
    fn cpu_off(&self) -> Result<(), ErrorCode> {
        let cpu_index = CoresImpl::core_index();
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let mut composite_state = PsciCompositePowerState::OFF;

        self.platform.power_domain_off_early(&composite_state)?;

        self.power_domain_tree
            .with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                self.forward_to_spm(Function::CpuOff);
                Self::get_spm().notify_cpu_off();
                cpu.set_local_state(PlatformPowerState::OFF);
                composite_state.coordinate_state(cpu_index, &mut ancestors);

                cpu_power_down(composite_state.find_highest_power_down_level().unwrap());

                self.platform.power_domain_off(&composite_state);
            });

        cpu.set_affinity_info(AffinityInfo::Off);

        // Unlock CPU before actually turning it off
        drop(cpu);

        self.platform.power_domain_power_down_wfi(&composite_state);
        // Does not return
    }

    /// Handles `CPU_ON` PSCI call by turning on the CPU identified by the given `target_cpu` MPIDR.
    /// The caller has to provide a valid non-secure entry point for the CPU.
    fn cpu_on(&self, target_cpu: Mpidr, entry: EntryPoint) -> Result<(), ErrorCode> {
        let cpu_index =
            try_get_cpu_index_by_mpidr(target_cpu).ok_or(ErrorCode::InvalidParameters)?;

        if !self.platform.is_valid_ns_entrypoint(&entry) {
            return Err(ErrorCode::InvalidAddress);
        }

        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        match cpu.affinity_info() {
            AffinityInfo::On => return Err(ErrorCode::AlreadyOn),
            AffinityInfo::OnPending => return Err(ErrorCode::OnPending),
            // The CPU was off, so continue CPU on operation.
            AffinityInfo::Off => {}
        }

        cpu.set_affinity_info(AffinityInfo::OnPending);

        match self.platform.power_domain_on(target_cpu) {
            Ok(_) => {
                cpu.set_entry_point(entry);
                Ok(())
            }
            Err(error) => {
                cpu.set_affinity_info(AffinityInfo::Off);
                Err(error)
            }
        }
    }

    /// This function must be called when a CPU is powered up. It returns the non-secure entry
    /// point and the reason why the CPU was powered up.
    pub fn handle_cpu_boot(&self) -> WakeUpReason {
        let cpu_index = CoresImpl::core_index();
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let mut composite_state = PsciCompositePowerState::RUN;
        let mut wake_from_suspend = false;

        let affinity_info = cpu.affinity_info();
        if affinity_info == AffinityInfo::Off {
            drop(cpu);
            panic!("Unexpected affinity info state");
        }

        let target_power_level = cpu
            .highest_affected_level()
            .unwrap_or(PsciPlatformImpl::MAX_POWER_LEVEL);

        self.power_domain_tree.with_ancestors_locked_to_max_level(
            &mut cpu,
            target_power_level,
            |cpu, mut ancestors| {
                composite_state.set_local_states_from_nodes(cpu, &ancestors);

                if affinity_info == AffinityInfo::OnPending {
                    // Finishing CPU_ON
                    self.platform.power_domain_on_finish(&composite_state);

                    cpu.set_affinity_info(AffinityInfo::On);
                } else {
                    // Waking up from suspend
                    assert_eq!(affinity_info, AffinityInfo::On);
                    assert_eq!(
                        composite_state.cpu_level_state().power_state_type(),
                        PowerStateType::PowerDown
                    );

                    self.platform.power_domain_suspend_finish(&composite_state);
                    cpu.clear_highest_affected_level();

                    wake_from_suspend = true;
                }

                cpu.set_local_state(PlatformPowerState::RUN);

                for node in ancestors.iter_mut() {
                    node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                    node.set_local_state(PlatformPowerState::RUN);
                }
            },
        );

        let entry_point = cpu.pop_entry_point();
        drop(cpu); // Unlock before possible panic

        let entry_point = entry_point.expect("entry point not set for booting CPU");

        if wake_from_suspend {
            WakeUpReason::SuspendFinished(entry_point)
        } else {
            WakeUpReason::CpuOn(entry_point)
        }
    }

    /// Handles `AFFINITY_INFO` PSCI call.
    fn affinity_info(
        &self,
        target_affinity: Mpidr,
        lowest_affinity_level: u32,
    ) -> Result<AffinityInfo, ErrorCode> {
        let cpu_index =
            try_get_cpu_index_by_mpidr(target_affinity).ok_or(ErrorCode::InvalidParameters)?;

        if lowest_affinity_level as usize > PsciCompositePowerState::CPU_POWER_LEVEL {
            // We don't support levels higher than CPU_POWER_LEVEL.
            return Err(ErrorCode::InvalidParameters);
        }

        Ok(self
            .power_domain_tree
            .locked_cpu_node(cpu_index)
            .affinity_info())
    }

    /// Handles `SYSTEM_OFF` PSCI call.
    /// Turns off the system and does not return.
    fn system_off(&self) -> ! {
        self.forward_to_spm(Function::SystemOff);
        self.platform.system_off();
    }

    /// Handles `SYSTEM_OFF2` PSCI call.
    /// Suspends system to disk and never returns on success.
    fn system_off2(&self, off_type: SystemOff2Type, cookie: Cookie) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::SYSTEM_OFF2) {
            return Err(ErrorCode::NotSupported);
        }

        self.forward_to_spm(Function::SystemOff2 { off_type, cookie });
        self.platform.system_off2(off_type, cookie)
    }

    /// Handles `SYSTEM_RESET` PSCI call.
    /// Resets the system and does not return.
    fn system_reset(&self) -> ! {
        self.forward_to_spm(Function::SystemReset);
        self.platform.system_reset();
    }

    /// Handles `SYSTEM_RESET2` PSCI call.
    /// Initiates an architectural or vendor specific system reset. Does not return on success.
    fn system_reset2(&self, reset_type: ResetType, cookie: Cookie) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::SYSTEM_RESET2) {
            return Err(ErrorCode::NotSupported);
        }

        self.forward_to_spm(Function::SystemReset2 { reset_type, cookie });
        self.platform.system_reset2(reset_type, cookie)
    }

    /// Handles `MEM_PROTECT` PSCI call.
    fn mem_protect(&self, enabled: bool) -> Result<bool, ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::MEM_PROTECT) {
            return Err(ErrorCode::NotSupported);
        }

        self.platform.mem_protect(enabled)
    }

    /// Handles `MEM_PROTECT_CHECK_RANGE` PSCI call.
    fn mem_protect_check_range(&self, range: MemProtectRange) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES
            .contains(PsciPlatformOptionalFeatures::MEM_PROTECT_CHECK_RANGE)
        {
            return Err(ErrorCode::NotSupported);
        }

        self.platform.mem_protect_check_range(range)
    }

    /// Handles `PSCI_FEATURES` PSCI call.
    fn handle_features(&self, feature: PsciFeature) -> Result<u64, ErrorCode> {
        const SUCCESS: u64 = 0;

        let check_optional_feature = |feature| {
            if PsciPlatformImpl::FEATURES.contains(feature) {
                Ok(SUCCESS)
            } else {
                Err(ErrorCode::NotSupported)
            }
        };

        match feature {
            PsciFeature::PsciFunction(function_id) => match function_id {
                // Mandatory features without feature flags
                FunctionId::PsciVersion
                | FunctionId::CpuOff
                | FunctionId::CpuOn32
                | FunctionId::CpuOn64
                | FunctionId::AffinityInfo32
                | FunctionId::AffinityInfo64
                | FunctionId::SystemOff
                | FunctionId::SystemReset
                | FunctionId::PsciFeatures => Ok(SUCCESS),

                // CPU suspend features
                FunctionId::CpuSuspend32 | FunctionId::CpuSuspend64 => {
                    let flags = FeatureFlagsCpuSuspend::EXTENDED_POWER_STATE
                        | (if PsciPlatformImpl::FEATURES
                            .contains(PsciPlatformOptionalFeatures::OS_INITIATED_MODE)
                        {
                            FeatureFlagsCpuSuspend::OS_INITIATED_MODE
                        } else {
                            FeatureFlagsCpuSuspend::empty()
                        });
                    Ok(u32::from(flags) as u64)
                }

                // Migrate
                FunctionId::Migrate32
                | FunctionId::Migrate64
                | FunctionId::MigrateInfoUpCpu32
                | FunctionId::MigrateInfoUpCpu64 => Err(ErrorCode::NotSupported),
                FunctionId::MigrateInfoType => Ok(SUCCESS),
                FunctionId::SystemOff232 | FunctionId::SystemOff264 => {
                    if PsciPlatformImpl::FEATURES
                        .contains(PsciPlatformOptionalFeatures::SYSTEM_OFF2)
                    {
                        let flags = FeatureFlagsSystemOff2::HIBERNATE_OFF;
                        Ok(u32::from(flags) as u64)
                    } else {
                        Err(ErrorCode::NotSupported)
                    }
                }
                FunctionId::SystemReset232 | FunctionId::SystemReset264 => {
                    check_optional_feature(PsciPlatformOptionalFeatures::SYSTEM_RESET2)
                }
                FunctionId::MemProtect => {
                    check_optional_feature(PsciPlatformOptionalFeatures::MEM_PROTECT)
                }
                FunctionId::MemProtectCheckRange32 | FunctionId::MemProtectCheckRange64 => {
                    check_optional_feature(PsciPlatformOptionalFeatures::MEM_PROTECT_CHECK_RANGE)
                }
                FunctionId::CpuFreeze => {
                    check_optional_feature(PsciPlatformOptionalFeatures::CPU_FREEZE)
                }
                FunctionId::CpuDefaultSuspend32 | FunctionId::CpuDefaultSuspend64 => {
                    check_optional_feature(PsciPlatformOptionalFeatures::CPU_DEFAULT_SUSPEND)
                }
                FunctionId::NodeHwState32 | FunctionId::NodeHwState64 => {
                    check_optional_feature(PsciPlatformOptionalFeatures::NODE_HW_STATE)
                }
                FunctionId::SystemSuspend32 | FunctionId::SystemSuspend64 => {
                    check_optional_feature(PsciPlatformOptionalFeatures::SYSTEM_SUSPEND)
                }
                FunctionId::PsciSetSuspendMode => {
                    check_optional_feature(PsciPlatformOptionalFeatures::OS_INITIATED_MODE)
                }
                FunctionId::PsciStatResidency32 | FunctionId::PsciStatResidency64 => {
                    Err(ErrorCode::NotSupported)
                }
                FunctionId::PsciStatCount32 | FunctionId::PsciStatCount64 => {
                    Err(ErrorCode::NotSupported)
                }
            },
            PsciFeature::SmcccVersion => Ok(SUCCESS),
        }
    }

    /// Handles `CPU_FREEZE` PSCI call.
    /// Does not return on success.
    fn cpu_freeze(&self) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::CPU_FREEZE) {
            return Err(ErrorCode::NotSupported);
        }

        self.platform.cpu_freeze()
    }

    /// Handles `CPU_DEFAULT_SUSPEND` PSCI call.
    /// Places a core into an implementation defined low-power state. It might not return if the
    /// default state is a power down state.
    fn cpu_default_suspend(&self, entry: EntryPoint) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::CPU_DEFAULT_SUSPEND) {
            return Err(ErrorCode::NotSupported);
        }

        let power_state = self.platform.cpu_default_suspend_power_state();
        self.cpu_suspend(power_state, entry)
    }

    /// Handles `NODE_HW_STATE` PSCI call.
    fn node_hw_state(&self, target_cpu: Mpidr, power_level: u32) -> Result<HwState, ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::NODE_HW_STATE) {
            return Err(ErrorCode::NotSupported);
        }

        if try_get_cpu_index_by_mpidr(target_cpu).is_none()
            || power_level as usize > PsciPlatformImpl::MAX_POWER_LEVEL
        {
            return Err(ErrorCode::InvalidParameters);
        }

        self.platform.node_hw_state(target_cpu, power_level)
    }

    /// Handles `SYSTEM_SUSPEND` PSCI call.
    /// Suspends system into RAM, does not return on success.
    fn system_suspend(&self, entry: EntryPoint) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::SYSTEM_SUSPEND) {
            return Err(ErrorCode::NotSupported);
        }

        let cpu_index = CoresImpl::core_index();

        if !self.power_domain_tree.is_last_cpu(cpu_index) {
            return Err(ErrorCode::Denied);
        }

        if !self.platform.is_valid_ns_entrypoint(&entry) {
            return Err(ErrorCode::InvalidAddress);
        }

        let state = self.platform.sys_suspend_power_state();
        if state.find_highest_non_run_level() != Some(PsciPlatformImpl::MAX_POWER_LEVEL) {
            return Err(ErrorCode::Denied);
        }

        assert!(state.is_valid_suspend_request(true));
        assert_eq!(
            state.highest_level_state().power_state_type(),
            PowerStateType::PowerDown
        );

        self.cpu_suspend_start(
            cpu_index,
            None,
            entry,
            PsciPlatformImpl::MAX_POWER_LEVEL,
            state,
            true,
        )
    }

    fn set_suspend_mode(&self, mode: SuspendMode) -> Result<u64, ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::OS_INITIATED_MODE) {
            return Err(ErrorCode::NotSupported);
        }
        if *self.suspend_mode.lock() == mode {
            return Ok(0);
        }
        match mode {
            SuspendMode::PlatformCoordinated => {
                if !self.power_domain_tree.is_last_cpu(CoresImpl::core_index()) {
                    return Err(ErrorCode::Denied);
                }
            }
            SuspendMode::OsInitiated => {
                if !(self.power_domain_tree.are_all_cpus_on()
                    || self.power_domain_tree.is_last_cpu(CoresImpl::core_index()))
                {
                    return Err(ErrorCode::Denied);
                }
            }
        }
        let mut suspend_mode_guard = self.suspend_mode.lock();
        *suspend_mode_guard = mode;
        Ok(0)
    }

    fn handle_smc_inner(&self, regs: &[u64; 4]) -> Result<u64, ErrorCode> {
        const SUCCESS: u64 = 0;
        let function = Function::try_from(regs)?;

        match function {
            Function::Version => {
                let version = Version { major: 1, minor: 3 };
                Ok(u32::from(version).into())
            }
            Function::CpuSuspend { state, entry } => {
                self.cpu_suspend(state, entry)?;
                Ok(SUCCESS)
            }
            Function::CpuOff => {
                self.cpu_off()?;
                Ok(SUCCESS)
            }
            Function::CpuOn { target_cpu, entry } => {
                self.cpu_on(target_cpu, entry)?;
                Ok(SUCCESS)
            }
            Function::AffinityInfo {
                mpidr,
                lowest_affinity_level,
            } => {
                let affinity_info = self.affinity_info(mpidr, lowest_affinity_level)?;
                Ok(u32::from(affinity_info).into())
            }
            Function::Migrate { .. } => Err(ErrorCode::NotSupported),
            Function::MigrateInfoType => {
                Ok(u32::from(MigrateInfoType::MigrationNotRequired).into())
            }
            Function::MigrateInfoUpCpu { .. } => Err(ErrorCode::NotSupported),

            Function::SystemOff => self.system_off(),
            Function::SystemOff2 { off_type, cookie } => {
                self.system_off2(off_type, cookie)?;
                Ok(SUCCESS)
            }
            Function::SystemReset => self.system_reset(),
            Function::SystemReset2 { reset_type, cookie } => {
                self.system_reset2(reset_type, cookie)?;
                Ok(SUCCESS)
            }
            Function::MemProtect { enabled } => {
                let previous_state = self.mem_protect(enabled)?;
                Ok(if previous_state { 1 } else { 0 })
            }
            Function::MemProtectCheckRange { range } => {
                self.mem_protect_check_range(range)?;
                Ok(SUCCESS)
            }
            Function::Features { psci_func_id } => self.handle_features(psci_func_id),
            Function::CpuFreeze => {
                self.cpu_freeze()?;
                Ok(SUCCESS)
            }
            Function::CpuDefaultSuspend { entry } => {
                self.cpu_default_suspend(entry)?;
                Ok(SUCCESS)
            }
            Function::NodeHwState {
                target_cpu,
                power_level,
            } => {
                let hw_state = self.node_hw_state(target_cpu, power_level)?;
                Ok(u32::from(hw_state).into())
            }
            Function::SystemSuspend { entry } => {
                self.system_suspend(entry)?;
                Ok(SUCCESS)
            }
            Function::SetSuspendMode { mode } => self.set_suspend_mode(mode),
            Function::StatResidency { .. } => Err(ErrorCode::NotSupported),
            Function::StatCount { .. } => Err(ErrorCode::NotSupported),
        }
    }

    #[cfg(not(test))]
    fn get_spm() -> &'static impl PsciSpmInterface {
        &Services::get().spmd
    }

    #[cfg(test)]
    fn get_spm() -> &'static impl PsciSpmInterface {
        &TestSpm
    }

    /// Forward a PSCI request to the SPM.
    fn forward_to_spm(&self, function: Function) {
        let mut psci_request = [0; 4];
        function.copy_to_array(&mut psci_request);

        let result = Self::get_spm().forward_psci_request(&psci_request);

        match ReturnCode::try_from(result as i32) {
            Ok(ReturnCode::Success) => {
                // Nothing to do
            }
            Ok(ReturnCode::Error(error_code)) => {
                // The SPM cannot prevent the PSCI state change, so we only log the error.
                log::error!("SPMD return {error_code:?} on PSCI event {function:?}")
            }
            Err(error) => log::error!("Failed to parse PSCI event response: {error:?}"),
        }
    }
}

impl Service for Psci {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        let in_regs: &mut [u64; 4] = (&mut regs.values_mut()[..4]).try_into().unwrap();
        let mut function = SmcFunctionId(in_regs[0] as u32);
        function.clear_sve_hint();
        in_regs[0] = function.0.into();

        let result: u64 = match self.handle_smc_inner(in_regs) {
            Ok(result) => result,
            Err(return_code) => return_code.into(),
        };

        regs.set_from(result);

        World::NonSecure
    }
}

impl Debug for Psci {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        self.power_domain_tree.fmt(f)
    }
}

/// Returns the corresponding linear core index for the given PSCI MPIDR value.
///
/// For any valid MPIDR this will return a unique value less than `Platform::CORE_COUNT`.
/// For any invalid MPIDR it will return `None`.
pub fn try_get_cpu_index_by_mpidr(psci_mpidr: Mpidr) -> Option<usize> {
    // The PSCI MPIDR value doesn't include the MT or U bits, but they might be important for how
    // the platform validates MPIDR values and calculates core position, so add them in.
    let mpidr = MpidrEl1::from_psci_mpidr(psci_mpidr.into());
    if PlatformImpl::mpidr_is_valid(mpidr) {
        Some(PlatformImpl::core_position(mpidr.bits()))
    } else {
        None
    }
}

#[cfg(not(test))]
unsafe extern "C" {
    pub unsafe fn bl31_warm_entrypoint();
}

#[cfg(test)]
mod tests {
    use super::*;
    use arm_psci::ArchitecturalResetType;
    use arm_sysregs::fake::SYSREGS;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    const ENTRY_POINT: EntryPoint = EntryPoint::Entry64 {
        entry_point_address: 0x0123_4567_89ab_cdef,
        context_id: 0xfedc_ba98_7654_3210,
    };

    const CPU0_MPIDR: Mpidr = Mpidr {
        aff0: 0,
        aff1: 0,
        aff2: 0,
        aff3: Some(0),
    };
    const CPU1_MPIDR: Mpidr = Mpidr {
        aff0: 1,
        aff1: 0,
        aff2: 0,
        aff3: Some(0),
    };
    const INVALID_MPIDR: Mpidr = Mpidr {
        aff0: 100,
        aff1: 100,
        aff2: 100,
        aff3: Some(100),
    };

    #[test]
    fn psci_composite_power_state() {
        let mut composite_state = PsciCompositePowerState::OFF;
        assert_eq!(PlatformPowerState::OFF, composite_state.cpu_level_state());

        assert_eq!(
            PlatformPowerState::OFF,
            composite_state.highest_level_state()
        );

        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::RUN;
        assert_eq!(PlatformPowerState::RUN, composite_state.cpu_level_state());

        composite_state = PsciCompositePowerState::OFF;
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL),
            composite_state.find_highest_power_down_level()
        );
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL),
            composite_state.find_highest_non_run_level()
        );

        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL] = PlatformPowerState::RUN;
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL - 1),
            composite_state.find_highest_power_down_level()
        );
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL - 1),
            composite_state.find_highest_non_run_level()
        );

        composite_state = PsciCompositePowerState::RUN;
        assert_eq!(None, composite_state.find_highest_power_down_level());
        assert_eq!(None, composite_state.find_highest_non_run_level());

        composite_state = PsciCompositePowerState::RUN;
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::OFF;
        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::RUN;
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::OFF;
        assert!(composite_state.is_valid_suspend_request(true));
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::RUN;
        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::OFF;
        assert!(composite_state.is_valid_suspend_request(true));
        assert!(!composite_state.is_valid_suspend_request(false));
    }

    #[test]
    fn psci_composite_power_state_set_from_nodes() {
        let mut composite_state = PsciCompositePowerState::OFF;
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        let mut cpu = tree.locked_cpu_node(2);
        tree.with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
            cpu.set_local_state(PlatformPowerState::RUN);
            for ancestor in ancestors.iter_mut() {
                ancestor.set_local_state(PlatformPowerState::RUN);
            }

            composite_state.set_local_states_from_nodes(cpu, &ancestors);
        });

        assert_eq!(
            [PlatformPowerState::RUN; {
                PsciPlatformImpl::MAX_POWER_LEVEL - PsciCompositePowerState::CPU_POWER_LEVEL + 1
            }],
            composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL..]
        )
    }

    #[test]
    fn psci_composite_power_state_coordination() {
        let mut composite_state = PsciCompositePowerState::OFF;
        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL - 1] = PlatformPowerState::RUN;
        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL] = PlatformPowerState::RUN;
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        let mut cpu = tree.locked_cpu_node(2);
        tree.with_ancestors_locked(&mut cpu, |_cpu, mut ancestors| {
            composite_state.coordinate_state(2, &mut ancestors);
        });
    }

    /// The function expects the closure to power down the calling CPU. This would normally end in
    /// a function which never returns (`func() -> !`). This makes it impossible to test it so this
    /// function introduces a method for unwinding the power down call and enables further testing.
    fn expect_cpu_power_down<F>(magic: &str, f: F)
    where
        F: Fn(),
    {
        // Run closure and expect panic unwind. AssertUnwindSafe is required, because spin::Mutex
        // does not implement UnwindSafe.
        let result = catch_unwind(AssertUnwindSafe(f));

        if let Err(err) = result {
            // The closure has panicked, check for power down magic string.
            if let Some(s) = err.downcast_ref::<String>()
                && *s == magic
            {
                return;
            }

            // Propagate non power down panics.
            resume_unwind(err);
        } else {
            // The closure finished without power down panic.
            panic!("Expected CPU power down did not happen");
        }
    }

    fn expect_cpu_power_down_wfi<F>(f: F)
    where
        F: Fn(),
    {
        expect_cpu_power_down(PsciPlatformImpl::POWER_DOWN_WFI_MAGIC, f);
    }

    #[test]
    fn psci_cpu_suspend() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.cpu_suspend(PowerState::StandbyOrRetention(100), ENTRY_POINT)
        );

        assert_eq!(
            Ok(()),
            psci.cpu_suspend(PowerState::StandbyOrRetention(0), ENTRY_POINT)
        );

        assert_eq!(
            Ok(()),
            psci.cpu_suspend(PowerState::StandbyOrRetention(2), ENTRY_POINT)
        );

        expect_cpu_power_down_wfi(|| {
            let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
        });

        let wakeup_reason = psci.handle_cpu_boot();
        assert_eq!(wakeup_reason, WakeUpReason::SuspendFinished(ENTRY_POINT));
    }

    #[test]
    fn psci_cpu_on() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.cpu_on(INVALID_MPIDR, ENTRY_POINT)
        );

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));
        assert_eq!(
            Err(ErrorCode::OnPending),
            psci.cpu_on(CPU1_MPIDR, ENTRY_POINT)
        );

        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(CPU1_MPIDR.into());
        let wakeup_reason = psci.handle_cpu_boot();
        assert_eq!(wakeup_reason, WakeUpReason::CpuOn(ENTRY_POINT));

        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(CPU0_MPIDR.into());
        assert_eq!(
            Err(ErrorCode::AlreadyOn),
            psci.cpu_on(CPU1_MPIDR, ENTRY_POINT)
        );

        SYSREGS.lock().unwrap().reset();
    }

    #[test]
    fn psci_cpu_off() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));

        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(CPU1_MPIDR.into());
        psci.handle_cpu_boot();

        expect_cpu_power_down_wfi(|| {
            let _ = psci.cpu_off();
        });

        SYSREGS.lock().unwrap().reset();
    }

    #[test]
    fn psci_affinity_info() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.affinity_info(
                INVALID_MPIDR,
                PsciCompositePowerState::CPU_POWER_LEVEL as u32
            )
        );

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.affinity_info(CPU1_MPIDR, PsciPlatformImpl::MAX_POWER_LEVEL as u32)
        );

        assert_eq!(
            Ok(AffinityInfo::Off),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));

        assert_eq!(
            Ok(AffinityInfo::OnPending),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );

        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(CPU1_MPIDR.into());
        let _entry_point = psci.handle_cpu_boot();
        assert_eq!(
            Ok(AffinityInfo::On),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );

        SYSREGS.lock().unwrap().reset();
    }

    fn check_ancestor_state(psci: &Psci, cpu_index: usize, expected_states: &[PlatformPowerState]) {
        let mut cpu = psci.power_domain_tree.locked_cpu_node(cpu_index);
        assert_eq!(expected_states[0], cpu.local_state());
        psci.power_domain_tree
            .with_ancestors_locked(&mut cpu, |_cpu, ancestors| {
                for (parent, expected_state) in ancestors.iter().zip(&expected_states[1..]) {
                    assert_eq!(*expected_state, parent.local_state());
                }
            });
    }

    #[test]
    fn psci_complex_suspend_scenario() {
        // Check correct state in the power domain tree while executing the following steps.
        // * Turn on all CPUs
        // * Power down CPUs 6-13
        // * Power down CPUs 0-5
        // * Wake up CPU0
        // * Wake up CPU6

        let cpus = [
            (0, 0, 0),
            (0, 0, 1),
            (0, 0, 2),
            (0, 1, 0),
            (0, 1, 1),
            (0, 1, 2),
            (1, 0, 0),
            (1, 0, 1),
            (1, 0, 2),
            (1, 1, 0),
            (1, 1, 1),
            (1, 1, 2),
            (1, 1, 3),
        ];

        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Ok(AffinityInfo::On),
            psci.affinity_info(
                Mpidr {
                    aff0: 0,
                    aff1: 0,
                    aff2: 0,
                    aff3: Some(0)
                },
                PsciCompositePowerState::CPU_POWER_LEVEL as u32
            )
        );

        // Turning on all secondary CPUs
        for cpu in &cpus[1..] {
            let mpidr = Mpidr {
                aff0: cpu.2,
                aff1: cpu.1,
                aff2: cpu.0,
                aff3: Some(0),
            };
            assert_eq!(Ok(()), psci.cpu_on(mpidr, ENTRY_POINT));
            assert_eq!(
                Ok(AffinityInfo::OnPending),
                psci.affinity_info(mpidr, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
            );
        }

        // Boot secondary CPUs
        for cpu in cpus.iter().skip(1) {
            let mpidr = Mpidr::from_aff3210(0, cpu.0, cpu.1, cpu.2);
            SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(mpidr.into());
            let wakeup_reason = psci.handle_cpu_boot();
            assert_eq!(wakeup_reason, WakeUpReason::CpuOn(ENTRY_POINT));
            assert_eq!(
                Ok(AffinityInfo::On),
                psci.affinity_info(mpidr, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
            );
        }

        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Put second cluster in power down suspend
        for cpu in cpus.iter().skip(6) {
            let mpidr = Mpidr::from_aff3210(0, cpu.0, cpu.1, cpu.2);
            SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(mpidr.into());
            expect_cpu_power_down_wfi(|| {
                let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
            });
        }

        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Power down off all other CPUs
        for cpu in cpus.iter().take(6) {
            let mpidr = Mpidr::from_aff3210(0, cpu.0, cpu.1, cpu.2);
            SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(mpidr.into());
            expect_cpu_power_down_wfi(|| {
                let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
            });
        }

        for cpu_index in 0..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                ],
            );
        }

        // Wake up CPU 0
        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(CPU0_MPIDR.into());
        let wakeup_reason = psci.handle_cpu_boot();
        assert_eq!(wakeup_reason, WakeUpReason::SuspendFinished(ENTRY_POINT));

        // First CPU is on
        check_ancestor_state(
            &psci,
            0,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // First cluster is on, but the CPUs are off
        for cpu_index in 1..3 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Second cluster is off
        for cpu_index in 3..6 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // The rest of the nodes are off except the top level node
        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Wake up CPU 6
        let cpu6_mpidr = Mpidr::from_aff3210(0, cpus[6].0, cpus[6].1, cpus[6].2);
        SYSREGS.lock().unwrap().mpidr_el1 = MpidrEl1::from_psci_mpidr(cpu6_mpidr.into());
        let wakeup_reason = psci.handle_cpu_boot();
        assert_eq!(wakeup_reason, WakeUpReason::SuspendFinished(ENTRY_POINT));

        // CPU 0 is still on
        check_ancestor_state(
            &psci,
            0,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // First cluster is on, but the CPUs are off
        for cpu_index in 1..3 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Second cluster is off
        for cpu_index in 3..6 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // CPU 6 is now on
        check_ancestor_state(
            &psci,
            6,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // Third cluster is on, but the CPUs are off
        for cpu_index in 7..9 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Fourth cluster is off
        for cpu_index in 9..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        SYSREGS.lock().unwrap().reset();
    }

    #[test]
    fn psci_system_off() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_OFF_MAGIC, || psci.system_off());
    }

    #[test]
    fn psci_system_off2() {
        let psci = Psci::new(PsciPlatformImpl::new());

        let off_type = SystemOff2Type::HibernateOff;
        let cookie = Cookie::Cookie64(0);

        let magic = format!(
            "{} {:?} {:?}",
            PsciPlatformImpl::SYSTEM_OFF2_MAGIC,
            off_type,
            cookie
        );

        expect_cpu_power_down(magic.as_str(), || {
            let _ = psci.system_off2(off_type, cookie);
        });
    }

    #[test]
    fn psci_system_reset() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_RESET_MAGIC, || psci.system_reset());
    }

    #[test]
    fn psci_system_reset2() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_RESET2_MAGIC, || {
            let _ = psci.system_reset2(
                ResetType::Architectural(ArchitecturalResetType::SystemWarmReset),
                Cookie::Cookie64(0),
            );
        });
    }

    #[test]
    fn psci_mem_protect() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(Ok(true), psci.mem_protect(true));
        assert_eq!(
            Ok(()),
            psci.mem_protect_check_range(MemProtectRange::Range64 { base: 0, length: 4 })
        );
    }

    #[test]
    fn psci_features() {
        let psci = Psci::new(PsciPlatformImpl::new());

        let supported_functions = [
            FunctionId::PsciVersion,
            FunctionId::CpuOff,
            FunctionId::CpuOn32,
            FunctionId::CpuOn64,
            FunctionId::AffinityInfo32,
            FunctionId::AffinityInfo64,
            FunctionId::SystemOff,
            FunctionId::SystemReset,
            FunctionId::SystemReset232,
            FunctionId::SystemReset264,
            FunctionId::MemProtect,
            FunctionId::MemProtectCheckRange32,
            FunctionId::MemProtectCheckRange64,
            FunctionId::PsciFeatures,
            FunctionId::PsciSetSuspendMode,
            FunctionId::CpuFreeze,
            FunctionId::CpuDefaultSuspend32,
            FunctionId::CpuDefaultSuspend64,
            FunctionId::NodeHwState32,
            FunctionId::NodeHwState64,
            FunctionId::SystemSuspend32,
            FunctionId::SystemSuspend64,
        ];

        let not_supported_functions = [
            FunctionId::Migrate32,
            FunctionId::Migrate64,
            FunctionId::MigrateInfoUpCpu32,
            FunctionId::MigrateInfoUpCpu64,
            FunctionId::PsciStatResidency32,
            FunctionId::PsciStatResidency64,
            FunctionId::PsciStatCount32,
            FunctionId::PsciStatCount64,
        ];

        assert_eq!(Ok(0), psci.handle_features(PsciFeature::SmcccVersion));
        assert_eq!(
            Ok(0x0000_0003),
            psci.handle_features(PsciFeature::PsciFunction(FunctionId::CpuSuspend32))
        );
        assert_eq!(
            Ok(0x0000_0003),
            psci.handle_features(PsciFeature::PsciFunction(FunctionId::CpuSuspend64))
        );
        assert_eq!(
            Ok(0x0000_0001),
            psci.handle_features(PsciFeature::PsciFunction(FunctionId::SystemOff232))
        );
        assert_eq!(
            Ok(0x0000_0001),
            psci.handle_features(PsciFeature::PsciFunction(FunctionId::SystemOff264))
        );
        assert_eq!(
            Ok(0),
            psci.handle_features(PsciFeature::PsciFunction(FunctionId::MigrateInfoType))
        );

        for function_id in supported_functions {
            assert_eq!(
                Ok(0),
                psci.handle_features(PsciFeature::PsciFunction(function_id))
            );
        }
        for function_id in not_supported_functions {
            assert_eq!(
                Err(ErrorCode::NotSupported),
                psci.handle_features(PsciFeature::PsciFunction(function_id))
            );
        }
    }

    #[test]
    fn psci_cpu_freeze() {
        let psci = Psci::new(PsciPlatformImpl::new());
        expect_cpu_power_down(PsciPlatformImpl::CPU_FREEZE_MAGIC, || {
            let _ = psci.cpu_freeze();
        });
    }

    #[test]
    fn psci_cpu_default_suspend() {
        let psci = Psci::new(PsciPlatformImpl::new());
        assert_eq!(Ok(()), psci.cpu_default_suspend(ENTRY_POINT));
    }

    #[test]
    fn psci_node_hw_state() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.node_hw_state(
                INVALID_MPIDR,
                PsciCompositePowerState::CPU_POWER_LEVEL as u32
            )
        );

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.node_hw_state(CPU1_MPIDR, PsciPlatformImpl::MAX_POWER_LEVEL as u32 + 1)
        );

        assert_eq!(
            Ok(HwState::Off),
            psci.node_hw_state(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );
    }

    #[test]
    fn psci_system_suspend() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down_wfi(|| {
            let _ = psci.system_suspend(ENTRY_POINT);
        });
        psci.handle_cpu_boot();

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));
        // Not last CPU
        assert_eq!(Err(ErrorCode::Denied), psci.system_suspend(ENTRY_POINT));
    }
}
