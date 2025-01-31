// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    platform::{exception_free, Platform, PlatformImpl},
    smccc::SmcReturn,
};
#[cfg(not(test))]
use core::arch::asm;
use core::{
    cell::{RefCell, RefMut},
    ptr::null_mut,
};
use percore::{ExceptionFree, ExceptionLock, PerCore};

/// The number of contexts to store for each CPU core, one per security state.
const CPU_DATA_CONTEXT_NUM: usize = if cfg!(feature = "rme") { 3 } else { 2 };

const CPU_DATA_CRASH_BUF_SIZE: usize = 64;

/// RES1 bits in the `scr_el3` register.
const SCR_RES1: u64 = 1 << 4 | 1 << 5;
const SCR_NS: u64 = 1 << 0;
const SCR_SIF: u64 = 1 << 9;
const SCR_RW: u64 = 1 << 10;
const SCR_EEL2: u64 = 1 << 18;

/// RES1 bits in the `sctlr_el1` register.
const SCTLR_EL1_RES1: u64 = 1 << 29 | 1 << 28 | 1 << 23 | 1 << 22 | 1 << 20 | 1 << 11;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum World {
    // The enum values must match those used by the `get_security_state` assembly function.
    Secure = 0,
    NonSecure = 1,
    #[cfg(feature = "rme")]
    Realm = 2,
}

impl World {
    fn index(self) -> usize {
        self as usize
    }
}

/// The state of a core at the next lower EL in a given security state.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CpuContext {
    pub gpregs: GpRegs,
    el3_state: El3State,
    el1_sysregs: El1Sysregs,
}

impl CpuContext {
    const EMPTY: Self = Self {
        gpregs: GpRegs::EMPTY,
        el3_state: El3State::EMPTY,
        el1_sysregs: El1Sysregs::EMPTY,
    };
}

/// AArch64 general purpose register context structure. Usually x0-x18 and lr are saved as the
/// compiler is expected to preserve the remaining callee saved registers if needed and the assembly
/// code does not touch the remaining. But in case of world switch during exception handling,
/// we need to save the callee registers too.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
pub struct GpRegs {
    pub registers: [u64; Self::COUNT],
}

impl GpRegs {
    /// The number of (64-bit) registers included in `GpRegs`.
    const COUNT: usize = 32;

    const EMPTY: Self = Self {
        registers: [0; Self::COUNT],
    };

    /// Writes the given return value to the general-purpose registers.
    pub fn write_return_value(&mut self, value: &SmcReturn) {
        for (i, value) in value.values().iter().enumerate() {
            self.registers[i] = *value;
        }
    }
}

/// Miscellaneous registers used by EL3 firmware to maintain its state across exception entries and
/// exits.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
struct El3State {
    scr_el3: u64,
    esr_el3: u64,
    runtime_sp: u64,
    spsr_el3: u64,
    elr_el3: u64,
    pmcr_el0: u64,
    is_in_el3: u64,
    saved_elr_el3: u64,
    nested_ea_flag: u64,
    _padding: u64,
}

impl El3State {
    const EMPTY: Self = Self {
        scr_el3: 0,
        esr_el3: 0,
        runtime_sp: 0,
        spsr_el3: 0,
        elr_el3: 0,
        pmcr_el0: 0,
        is_in_el3: 0,
        saved_elr_el3: 0,
        nested_ea_flag: 0,
        _padding: 0,
    };
}

/// AArch64 EL1 system register context structure for preserving the architectural state during
/// world switches.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
struct El1Sysregs {
    spsr_el1: u64,
    elr_el1: u64,
    sctlr_el1: u64,
    tcr_el1: u64,
    cpacr_el1: u64,
    csselr_el1: u64,
    sp_el1: u64,
    esr_el1: u64,
    ttbr0_el1: u64,
    ttbr1_el1: u64,
    mair_el1: u64,
    amair_el1: u64,
    actlr_el1: u64,
    tpidr_el1: u64,
    tpidr_el0: u64,
    tpidrro_el0: u64,
    par_el1: u64,
    far_el1: u64,
    afsr0_el1: u64,
    afsr1_el1: u64,
    contextidr_el1: u64,
    vbar_el1: u64,
    mdccint_el1: u64,
    mdscr_el1: u64,
}

impl El1Sysregs {
    const EMPTY: Self = Self {
        spsr_el1: 0,
        elr_el1: 0,
        sctlr_el1: 0,
        tcr_el1: 0,
        cpacr_el1: 0,
        csselr_el1: 0,
        sp_el1: 0,
        esr_el1: 0,
        ttbr0_el1: 0,
        ttbr1_el1: 0,
        mair_el1: 0,
        amair_el1: 0,
        actlr_el1: 0,
        tpidr_el1: 0,
        tpidr_el0: 0,
        tpidrro_el0: 0,
        par_el1: 0,
        far_el1: 0,
        afsr0_el1: 0,
        afsr1_el1: 0,
        contextidr_el1: 0,
        vbar_el1: 0,
        mdccint_el1: 0,
        mdscr_el1: 0,
    };
}

/// Registers whose values can be shared across CPUs.
#[derive(Clone, Debug, Default)]
#[repr(C)]
struct PerWorldContext {
    cptr_el3: u64,
    zcr_el3: u64,
}

impl PerWorldContext {
    const EMPTY: Self = Self {
        cptr_el3: 0,
        zcr_el3: 0,
    };
}

#[derive(Clone, Debug)]
#[repr(C, align(64))]
struct CpuData {
    cpu_context: [*mut u8; CPU_DATA_CONTEXT_NUM],
    cpu_ops_ptr: usize,
    psci_svc_cpu_data: PsciCpuData,
    crash_buf: [u64; CPU_DATA_CRASH_BUF_SIZE >> 3],
}

impl CpuData {
    const EMPTY: Self = Self {
        cpu_context: [null_mut(); CPU_DATA_CONTEXT_NUM],
        cpu_ops_ptr: 0,
        psci_svc_cpu_data: PsciCpuData {
            aff_info_state: AffInfoState::On,
            target_pwrlvl: 0,
            local_state: 0,
        },
        crash_buf: [0; CPU_DATA_CRASH_BUF_SIZE >> 3],
    };
}

#[derive(Clone, Debug)]
#[repr(C)]
struct PsciCpuData {
    aff_info_state: AffInfoState,
    target_pwrlvl: u32,
    local_state: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
enum AffInfoState {
    On = 0,
    Off = 1,
    OnPending = 2,
}

#[unsafe(export_name = "per_world_context")]
static mut PER_WORLD_CONTEXT: [PerWorldContext; CPU_DATA_CONTEXT_NUM] =
    [PerWorldContext::EMPTY; CPU_DATA_CONTEXT_NUM];

#[unsafe(export_name = "percpu_data")]
static mut PERCPU_DATA: [CpuData; PlatformImpl::CORE_COUNT] =
    [CpuData::EMPTY; PlatformImpl::CORE_COUNT];

#[derive(Debug)]
pub struct CpuState {
    cpu_contexts: [CpuContext; CPU_DATA_CONTEXT_NUM],
}

impl CpuState {
    const EMPTY: Self = Self {
        cpu_contexts: [CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
    };

    pub fn context(&self, world: World) -> &CpuContext {
        &self.cpu_contexts[world.index()]
    }

    pub fn context_mut(&mut self, world: World) -> &mut CpuContext {
        &mut self.cpu_contexts[world.index()]
    }
}

static CPU_STATE: PerCore<
    ExceptionLock<RefCell<CpuState>>,
    PlatformImpl,
    { PlatformImpl::CORE_COUNT },
> = PerCore::new(
    [const { ExceptionLock::new(RefCell::new(CpuState::EMPTY)) }; PlatformImpl::CORE_COUNT],
);

/// Sets SP_EL3 to a pointer to the given CpuContext, ready for exception return.
///
/// # Safety
///
/// The given context pointer must remain valid until a new next context is set.
unsafe fn set_next_context(context: *mut CpuContext) {
    // SAFETY: The caller guarantees that the context remains valid until it's replaced.
    #[cfg(not(test))]
    unsafe {
        asm!(
            "msr spsel, #1",
            "mov sp, {context}",
            "msr spsel, #0",
            context = in(reg) context,
        )
    }
}

/// Selects the given world to run on the next exception return.
///
/// This works by setting `SP_EL3` to point to the appropriate `CpuContext` struct, so the
/// exception return code will restore registers from it before the `eret`.
pub fn set_next_world_context(world: World) {
    // SAFETY: The CPU context is always valid, and will only be used via this pointer by assembly
    // code after the Rust code returns to prepare for the eret, and after the next exception before
    // entering the Rust code again.
    unsafe { set_next_context(&raw mut (*CPU_STATE.get().as_ptr()).cpu_contexts[world.index()]) }
}

/// Returns a reference to the `CpuState` for the current CPU.
///
/// Panics if the `CpuState` is already borrowed.
pub fn cpu_state(token: ExceptionFree) -> RefMut<CpuState> {
    CPU_STATE.get().borrow_mut(token)
}

/// Initialises all CPU contexts for this CPU, ready for first boot.
pub fn initialise_contexts(
    non_secure_entry_point: &EntryPointInfo,
    secure_entry_point: &EntryPointInfo,
) {
    exception_free(|token| {
        initialise_nonsecure(
            cpu_state(token).context_mut(World::NonSecure),
            non_secure_entry_point,
        );
        initialise_secure(
            cpu_state(token).context_mut(World::Secure),
            secure_entry_point,
        );
    });
}

/// Initialises parts of the given CPU context that are the same for all worlds.
fn initialise_common(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    context.el3_state.elr_el3 = entry_point.pc;
    context.el3_state.spsr_el3 = entry_point.spsr;
    context.el3_state.scr_el3 = SCR_RES1 | SCR_SIF | SCR_RW | SCR_EEL2;
    context.el1_sysregs.sctlr_el1 = SCTLR_EL1_RES1;
    // TODO: Initialise EL2 state too.
}

/// Initialises the given CPU context ready for booting NS-EL2 or NS-EL1.
fn initialise_nonsecure(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);
    context.el3_state.scr_el3 |= SCR_NS;
    // TODO: FIQ and IRQ routing model.
}

/// Initialises the given CPU context ready for booting S-EL2 or S-EL1.
fn initialise_secure(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);
    // TODO: FIQ and IRQ routing model.
}

/// Information about the entry point for a next stage (e.g. BL32 or BL33).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryPointInfo {
    /// The entry point address.
    pub pc: u64,
    /// The `spsr_el3` value to set before `eret`, to set the appropriate PSTATE.
    pub spsr: u64,
    /// Boot arguments to pass in `x0`-`x7`.
    pub args: [u64; 8],
}
