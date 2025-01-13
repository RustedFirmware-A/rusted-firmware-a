// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    platform::{exception_free, Platform, PlatformImpl},
    smccc::SmcReturn,
    sysregs::{
        read_actlr_el2, read_afsr0_el2, read_afsr1_el2, read_amair_el2, read_cnthctl_el2,
        read_cntvoff_el2, read_cptr_el2, read_elr_el2, read_esr_el2, read_far_el2, read_hacr_el2,
        read_hcr_el2, read_hpfar_el2, read_hstr_el2, read_icc_sre_el2, read_ich_hcr_el2,
        read_ich_vmcr_el2, read_mair_el2, read_mdcr_el2, read_scr_el3, read_sctlr_el2, read_sp_el2,
        read_spsr_el2, read_tcr_el2, read_tpidr_el2, read_ttbr0_el2, read_vbar_el2,
        read_vmpidr_el2, read_vpidr_el2, read_vtcr_el2, read_vttbr_el2, write_actlr_el2,
        write_afsr0_el2, write_afsr1_el2, write_amair_el2, write_cnthctl_el2, write_cntvoff_el2,
        write_cptr_el2, write_elr_el2, write_esr_el2, write_far_el2, write_hacr_el2, write_hcr_el2,
        write_hpfar_el2, write_hstr_el2, write_icc_sre_el2, write_icc_sre_el3, write_ich_hcr_el2,
        write_mair_el2, write_mdcr_el2, write_scr_el3, write_sctlr_el2, write_sp_el2, write_sp_el3,
        write_spsr_el2, write_tcr_el2, write_tpidr_el2, write_ttbr0_el2, write_vbar_el2,
        write_vmpidr_el2, write_vpidr_el2, write_vtcr_el2, write_vttbr_el2,
    },
};
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
pub const SCR_NS: u64 = 1 << 0;
const SCR_EA: u64 = 1 << 3;
const SCR_HCE: u64 = 1 << 8;
const SCR_SIF: u64 = 1 << 9;
const SCR_RW: u64 = 1 << 10;
const SCR_EEL2: u64 = 1 << 18;
const SCR_NSE: u64 = 1 << 62;

/// RES1 bits in the `sctlr_el1` register.
const SCTLR_EL1_RES1: u64 = 1 << 29 | 1 << 28 | 1 << 23 | 1 << 22 | 1 << 20 | 1 << 11;

// Bits for the `icc_sre_el2` and `icc_sre_el3` registers.
const ICC_SRE_SRE: u64 = 1 << 0;
const ICC_SRE_DFB: u64 = 1 << 1;
const ICC_SRE_DIB: u64 = 1 << 2;
const ICC_SRE_EN: u64 = 1 << 3;

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

    /// Reads the current lower EL world from `scr_el3`.
    pub fn from_scr() -> Self {
        let scr_el3 = read_scr_el3();
        match (scr_el3 & SCR_NSE != 0, scr_el3 & SCR_NS != 0) {
            (false, false) => World::Secure,
            (false, true) => World::NonSecure,
            #[cfg(feature = "rme")]
            (true, true) => World::Realm,
            _ => panic!("Invalid combination of NS and NSE in scr_el3"),
        }
    }
}

/// The state of a core at the next lower EL in a given security state.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CpuContext {
    pub gpregs: GpRegs,
    el3_state: El3State,
    el2_sysregs: El2Sysregs,
    el1_sysregs: El1Sysregs,
}

impl CpuContext {
    const EMPTY: Self = Self {
        gpregs: GpRegs::EMPTY,
        el3_state: El3State::EMPTY,
        el2_sysregs: El2Sysregs::EMPTY,
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

/// AArch64 EL2 system register context structure for preserving the architectural state during
/// world switches.
#[derive(Clone, Debug, Eq, PartialEq)]
struct El2Sysregs {
    actlr_el2: u64,
    afsr0_el2: u64,
    afsr1_el2: u64,
    amair_el2: u64,
    cnthctl_el2: u64,
    cntvoff_el2: u64,
    cptr_el2: u64,
    elr_el2: u64,
    esr_el2: u64,
    far_el2: u64,
    hacr_el2: u64,
    hcr_el2: u64,
    hpfar_el2: u64,
    hstr_el2: u64,
    icc_sre_el2: u64,
    ich_hcr_el2: u64,
    ich_vmcr_el2: u64,
    mair_el2: u64,
    mdcr_el2: u64,
    sctlr_el2: u64,
    spsr_el2: u64,
    sp_el2: u64,
    tcr_el2: u64,
    tpidr_el2: u64,
    ttbr0_el2: u64,
    vbar_el2: u64,
    vmpidr_el2: u64,
    vpidr_el2: u64,
    vtcr_el2: u64,
    vttbr_el2: u64,
}

impl El2Sysregs {
    const EMPTY: Self = Self {
        actlr_el2: 0,
        afsr0_el2: 0,
        afsr1_el2: 0,
        amair_el2: 0,
        cnthctl_el2: 0,
        cntvoff_el2: 0,
        cptr_el2: 0,
        elr_el2: 0,
        esr_el2: 0,
        far_el2: 0,
        hacr_el2: 0,
        hcr_el2: 0,
        hpfar_el2: 0,
        hstr_el2: 0,
        icc_sre_el2: 0,
        ich_hcr_el2: 0,
        ich_vmcr_el2: 0,
        mair_el2: 0,
        mdcr_el2: 0,
        sctlr_el2: 0,
        spsr_el2: 0,
        sp_el2: 0,
        tcr_el2: 0,
        tpidr_el2: 0,
        ttbr0_el2: 0,
        vbar_el2: 0,
        vmpidr_el2: 0,
        vpidr_el2: 0,
        vtcr_el2: 0,
        vttbr_el2: 0,
    };

    /// Reads the current values from the system registers to save them.
    fn save(&mut self) {
        self.actlr_el2 = read_actlr_el2();
        self.afsr0_el2 = read_afsr0_el2();
        self.afsr1_el2 = read_afsr1_el2();
        self.amair_el2 = read_amair_el2();
        self.cnthctl_el2 = read_cnthctl_el2();
        self.cntvoff_el2 = read_cntvoff_el2();
        self.cptr_el2 = read_cptr_el2();
        self.elr_el2 = read_elr_el2();
        self.esr_el2 = read_esr_el2();
        self.far_el2 = read_far_el2();
        self.hacr_el2 = read_hacr_el2();
        self.hcr_el2 = read_hcr_el2();
        self.hpfar_el2 = read_hpfar_el2();
        self.hstr_el2 = read_hstr_el2();
        self.icc_sre_el2 = read_icc_sre_el2();
        self.ich_hcr_el2 = read_ich_hcr_el2();
        self.ich_vmcr_el2 = read_ich_vmcr_el2();
        self.mair_el2 = read_mair_el2();
        self.mdcr_el2 = read_mdcr_el2();
        self.sctlr_el2 = read_sctlr_el2();
        self.spsr_el2 = read_spsr_el2();
        self.sp_el2 = read_sp_el2();
        self.tcr_el2 = read_tcr_el2();
        self.tpidr_el2 = read_tpidr_el2();
        self.ttbr0_el2 = read_ttbr0_el2();
        self.vbar_el2 = read_vbar_el2();
        self.vmpidr_el2 = read_vmpidr_el2();
        self.vpidr_el2 = read_vpidr_el2();
        self.vtcr_el2 = read_vtcr_el2();
        self.vttbr_el2 = read_vttbr_el2();
    }

    /// Writes the saved register values to the system registers.
    fn restore(&self) {
        write_actlr_el2(self.actlr_el2);
        write_afsr0_el2(self.afsr0_el2);
        write_afsr1_el2(self.afsr1_el2);
        write_amair_el2(self.amair_el2);
        write_cnthctl_el2(self.cnthctl_el2);
        write_cntvoff_el2(self.cntvoff_el2);
        write_cptr_el2(self.cptr_el2);
        write_elr_el2(self.elr_el2);
        write_esr_el2(self.esr_el2);
        write_far_el2(self.far_el2);
        write_hacr_el2(self.hacr_el2);
        write_hcr_el2(self.hcr_el2);
        write_hpfar_el2(self.hpfar_el2);
        write_hstr_el2(self.hstr_el2);
        write_icc_sre_el2(self.icc_sre_el2);
        write_ich_hcr_el2(self.ich_hcr_el2);
        // TODO: Write the ich_vmcr_el2 register when the GIC driver is used
        // write_ich_vmcr_el2(self.ich_vmcr_el2);
        write_mair_el2(self.mair_el2);
        write_mdcr_el2(self.mdcr_el2);
        write_sctlr_el2(self.sctlr_el2);
        write_spsr_el2(self.spsr_el2);
        write_sp_el2(self.sp_el2);
        write_tcr_el2(self.tcr_el2);
        write_tpidr_el2(self.tpidr_el2);
        write_ttbr0_el2(self.ttbr0_el2);
        write_vbar_el2(self.vbar_el2);
        write_vmpidr_el2(self.vmpidr_el2);
        write_vpidr_el2(self.vpidr_el2);
        write_vtcr_el2(self.vtcr_el2);
        write_vttbr_el2(self.vttbr_el2);
    }
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
    unsafe {
        write_sp_el3(context as usize);
    }
}

/// Selects the given world to run on the next exception return.
///
/// This works by setting `SP_EL3` to point to the appropriate `CpuContext` struct, so the
/// exception return code will restore registers from it before the `eret`.
fn set_next_world_context(world: World) {
    // SAFETY: The CPU context is always valid, and will only be used via this pointer by assembly
    // code after the Rust code returns to prepare for the eret, and after the next exception before
    // entering the Rust code again.
    unsafe { set_next_context(&raw mut (*CPU_STATE.get().as_ptr()).cpu_contexts[world.index()]) }
}

/// Saves lower EL system registers from the current world, restores lower EL system registers of
/// the given world, and sets the world to run on the next exception return.
pub fn switch_world(old_world: World, new_world: World) {
    assert_ne!(old_world, new_world);
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        cpu_state.context_mut(old_world).el2_sysregs.save();
        cpu_state.context_mut(new_world).el2_sysregs.restore();
    });
    set_next_world_context(new_world);
}

/// Restores lower EL system registers of the given world and then sets it as the world to run on
/// the next exception return.
///
/// This doesn't save the current state of the lower EL system registers, so should only be used for
/// initial boot where we don't care about their state.
pub fn set_initial_world(world: World) {
    // ICC_SRE_EL3 must be set to 0xF before configuring ICC_SRE_EL2
    // TODO: Remove when GIC driver is used/implemented, as this should be set by the driver.
    //       As `set_initial_world` gets called only during initial boot, we place it here until
    //       it gets removed when the GIC driver gets added.
    // SAFETY: This is the only place we set `icc_sre_el3`, and we set the SRE bit, so it is never
    // changed from 1 to 0.
    unsafe {
        write_icc_sre_el3(ICC_SRE_DIB | ICC_SRE_DFB | ICC_SRE_EN | ICC_SRE_SRE);
    }

    exception_free(|token| {
        let cpu_state = cpu_state(token);
        let context = cpu_state.context(world);

        // This must be initialised before the EL2 system registers are written to, to avoid an
        // exception.
        write_scr_el3(context.el3_state.scr_el3);
        context.el2_sysregs.restore();
    });
    set_next_world_context(world);
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
    #[cfg(feature = "rme")] realm_entry_point: &EntryPointInfo,
) {
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        initialise_nonsecure(
            cpu_state.context_mut(World::NonSecure),
            non_secure_entry_point,
        );
        initialise_secure(cpu_state.context_mut(World::Secure), secure_entry_point);
        #[cfg(feature = "rme")]
        initialise_realm(cpu_state.context_mut(World::Realm), realm_entry_point);
    });
}

/// Initialises parts of the given CPU context that are the same for all worlds.
fn initialise_common(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    context.el3_state.elr_el3 = entry_point.pc;
    context.el3_state.spsr_el3 = entry_point.spsr;
    context.gpregs.registers[..entry_point.args.len()].copy_from_slice(&entry_point.args);

    // Initialise SCR_EL3, setting all fields rather than relying on hw.
    // All fields are architecturally UNKNOWN on reset.
    // The following fields do not change during the TF lifetime.
    //
    // SCR_EL3.TWE: Set to zero so that execution of WFE instructions at
    // EL2, EL1 and EL0 are not trapped to EL3.
    //
    // SCR_EL3.TWI: Set to zero so that execution of WFI instructions at
    // EL2, EL1 and EL0 are not trapped to EL3.
    //
    // SCR_EL3.SIF: Set to one to disable instruction fetches from
    // Non-secure memory.
    // SCR_EL3.SMD: Set to zero to enable SMC calls at EL1 and above, from
    // both Security states and both Execution states.
    //
    // SCR_EL3.EA: Set to one to route External Aborts and SError Interrupts
    // to EL3 when executing at any EL.
    //
    // SCR_EL3.EEL2: Set to one if S-EL2 is present and enabled.
    //
    // NOTE: Modifying EEL2 bit along with EA bit ensures that we mitigate
    // aganst ERRATA_V2_3099206.
    context.el3_state.scr_el3 = SCR_RES1 | SCR_HCE | SCR_EA | SCR_SIF | SCR_EEL2 | SCR_RW;
    context.el1_sysregs.sctlr_el1 = SCTLR_EL1_RES1;
    // TODO: Initialise the rest of the context.el2_sysregs too.
    context.el2_sysregs.icc_sre_el2 = ICC_SRE_DIB | ICC_SRE_DFB | ICC_SRE_EN | ICC_SRE_SRE;
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

/// Initialises the given CPU context ready for booting Realm world
#[cfg(feature = "rme")]
fn initialise_realm(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);
    // SCR_NS + SCR_NSE = Realm state
    context.el3_state.scr_el3 |= SCR_NS | SCR_NSE;
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
