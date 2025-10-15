// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    aarch64::isb,
    gicv3,
    platform::{Platform, PlatformImpl, exception_free},
    smccc::SmcReturn,
    sysregs::is_feat_vhe_present,
};
use arm_psci::EntryPoint;
use arm_sysregs::read_mpidr_el1;
use arm_sysregs::{CptrEl3, Esr, ScrEl3, Spsr, write_scr_el3};
#[cfg(feature = "sel2")]
use arm_sysregs::{
    HcrEl2, IccSre, read_actlr_el2, read_afsr0_el2, read_afsr1_el2, read_amair_el2,
    read_cnthctl_el2, read_cntvoff_el2, read_contextidr_el2, read_cptr_el2, read_elr_el2,
    read_esr_el2, read_far_el2, read_hacr_el2, read_hcr_el2, read_hpfar_el2, read_hstr_el2,
    read_icc_sre_el2, read_ich_hcr_el2, read_ich_vmcr_el2, read_mair_el2, read_mdcr_el2,
    read_sctlr_el2, read_sp_el2, read_spsr_el2, read_tcr_el2, read_tpidr_el2, read_ttbr0_el2,
    read_ttbr1_el2, read_vbar_el2, read_vmpidr_el2, read_vpidr_el2, read_vtcr_el2, read_vttbr_el2,
    write_actlr_el2, write_afsr0_el2, write_afsr1_el2, write_amair_el2, write_cnthctl_el2,
    write_cntvoff_el2, write_contextidr_el2, write_cptr_el2, write_elr_el2, write_esr_el2,
    write_far_el2, write_hacr_el2, write_hcr_el2, write_hpfar_el2, write_hstr_el2,
    write_icc_sre_el2, write_ich_hcr_el2, write_mair_el2, write_mdcr_el2, write_sctlr_el2,
    write_sp_el2, write_spsr_el2, write_tcr_el2, write_tpidr_el2, write_ttbr0_el2, write_ttbr1_el2,
    write_vbar_el2, write_vmpidr_el2, write_vpidr_el2, write_vtcr_el2, write_vttbr_el2,
};
#[cfg(not(feature = "sel2"))]
use arm_sysregs::{
    SctlrEl1, read_actlr_el1, read_afsr0_el1, read_afsr1_el1, read_amair_el1, read_contextidr_el1,
    read_cpacr_el1, read_csselr_el1, read_elr_el1, read_esr_el1, read_far_el1, read_mair_el1,
    read_mdccint_el1, read_mdscr_el1, read_par_el1, read_sctlr_el1, read_sp_el1, read_spsr_el1,
    read_tcr_el1, read_tpidr_el0, read_tpidr_el1, read_tpidrro_el0, read_ttbr0_el1, read_ttbr1_el1,
    read_vbar_el1, write_actlr_el1, write_afsr0_el1, write_afsr1_el1, write_amair_el1,
    write_contextidr_el1, write_cpacr_el1, write_csselr_el1, write_elr_el1, write_esr_el1,
    write_far_el1, write_mair_el1, write_mdccint_el1, write_mdscr_el1, write_par_el1,
    write_sctlr_el1, write_sp_el1, write_spsr_el1, write_tcr_el1, write_tpidr_el0, write_tpidr_el1,
    write_tpidrro_el0, write_ttbr0_el1, write_ttbr1_el1, write_vbar_el1,
};
use core::{
    cell::{RefCell, RefMut},
    ops::{Index, IndexMut},
    ptr::{null, null_mut},
};
use percore::{Cores, ExceptionFree, ExceptionLock, PerCore};

/// The number of contexts to store for each CPU core, one per security state.
const CPU_DATA_CONTEXT_NUM: usize = if cfg!(feature = "rme") { 3 } else { 2 };

/// The number of registers which can be saved in the crash buffer.
const CPU_DATA_CRASH_BUF_COUNT: usize = 8;

/// Per-core mutable state.
pub type PerCoreState<T> =
    PerCore<ExceptionLock<RefCell<T>>, CoresImpl, { PlatformImpl::CORE_COUNT }>;

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

/// Implementation of the `Cores` trait to get the index of the current CPU core.
pub struct CoresImpl;

// SAFETY: This implementation never returns the same index for different cores because
// `core_position` is guaranteed not to.
unsafe impl Cores for CoresImpl {
    fn core_index() -> usize {
        PlatformImpl::core_position(read_mpidr_el1().bits())
    }
}

/// The state of a core at the next lower EL in a given security state.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CpuContext {
    pub gpregs: GpRegs,
    pub el3_state: El3State,
    #[cfg(feature = "sel2")]
    el2_sysregs: El2Sysregs,
    #[cfg(not(feature = "sel2"))]
    el1_sysregs: El1Sysregs,
}

impl CpuContext {
    const EMPTY: Self = Self {
        gpregs: GpRegs::EMPTY,
        el3_state: El3State::EMPTY,
        #[cfg(feature = "sel2")]
        el2_sysregs: El2Sysregs::EMPTY,
        #[cfg(not(feature = "sel2"))]
        el1_sysregs: El1Sysregs::EMPTY,
    };

    fn save_lower_el_sysregs(&mut self) {
        #[cfg(feature = "sel2")]
        self.el2_sysregs.save();
        #[cfg(not(feature = "sel2"))]
        self.el1_sysregs.save();
    }

    fn restore_lower_el_sysregs(&self) {
        #[cfg(feature = "sel2")]
        self.el2_sysregs.restore();
        #[cfg(not(feature = "sel2"))]
        self.el1_sysregs.restore();
    }

    /// Skips an instruction in a lower EL.
    ///
    /// Increases ELR_EL3 in the saved context by the size of an instruction. After exception return
    /// the execution in lower EL will continue from the next instruction instead of repeating the
    /// one that has caused the trap.
    /// Should only be used by [`crate::services::Services::handle_sysreg_trap()`].
    pub fn skip_lower_el_instruction(&mut self) {
        self.el3_state.elr_el3 += core::mem::size_of::<u32>();
    }
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
pub struct El3State {
    pub scr_el3: ScrEl3,
    esr_el3: Esr,
    // The runtime_sp and runtime_lr fields must be adjacent, because assembly code uses ldp/stp
    // instructions to load/store these together.
    runtime_sp: u64,
    runtime_lr: u64,
    pub spsr_el3: Spsr,
    pub elr_el3: usize,
    pmcr_el0: u64,
    is_in_el3: u64,
    saved_elr_el3: u64,
    nested_ea_flag: u64,
}

impl El3State {
    const EMPTY: Self = Self {
        scr_el3: ScrEl3::empty(),
        esr_el3: Esr::empty(),
        runtime_sp: 0,
        runtime_lr: 0,
        spsr_el3: Spsr::empty(),
        elr_el3: 0,
        pmcr_el0: 0,
        is_in_el3: 0,
        saved_elr_el3: 0,
        nested_ea_flag: 0,
    };
}

/// AArch64 EL1 system register context structure for preserving the architectural state during
/// world switches.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(not(feature = "sel2"))]
struct El1Sysregs {
    spsr_el1: Spsr,
    elr_el1: usize,
    sctlr_el1: SctlrEl1,
    tcr_el1: u64,
    cpacr_el1: u64,
    csselr_el1: u64,
    sp_el1: u64,
    esr_el1: Esr,
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
    vbar_el1: usize,
    mdccint_el1: u64,
    mdscr_el1: u64,
}

#[cfg(not(feature = "sel2"))]
impl El1Sysregs {
    const EMPTY: Self = Self {
        spsr_el1: Spsr::empty(),
        elr_el1: 0,
        sctlr_el1: SctlrEl1::empty(),
        tcr_el1: 0,
        cpacr_el1: 0,
        csselr_el1: 0,
        sp_el1: 0,
        esr_el1: Esr::empty(),
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

    /// Reads the current values from the system registers to save them.
    fn save(&mut self) {
        self.spsr_el1 = read_spsr_el1();
        self.elr_el1 = read_elr_el1();
        self.sctlr_el1 = read_sctlr_el1();
        self.tcr_el1 = read_tcr_el1();
        self.cpacr_el1 = read_cpacr_el1();
        self.csselr_el1 = read_csselr_el1();
        self.sp_el1 = read_sp_el1();
        self.esr_el1 = read_esr_el1();
        self.ttbr0_el1 = read_ttbr0_el1();
        self.ttbr1_el1 = read_ttbr1_el1();
        self.mair_el1 = read_mair_el1();
        self.amair_el1 = read_amair_el1();
        self.actlr_el1 = read_actlr_el1();
        self.tpidr_el1 = read_tpidr_el1();
        self.tpidr_el0 = read_tpidr_el0();
        self.tpidrro_el0 = read_tpidrro_el0();
        self.par_el1 = read_par_el1();
        self.far_el1 = read_far_el1();
        self.afsr0_el1 = read_afsr0_el1();
        self.afsr1_el1 = read_afsr1_el1();
        self.contextidr_el1 = read_contextidr_el1();
        self.vbar_el1 = read_vbar_el1();
        self.mdccint_el1 = read_mdccint_el1();
        self.mdscr_el1 = read_mdscr_el1();
    }

    /// Writes the saved register values to the system registers.
    fn restore(&self) {
        write_spsr_el1(self.spsr_el1);
        write_elr_el1(self.elr_el1);
        write_sctlr_el1(self.sctlr_el1);
        write_tcr_el1(self.tcr_el1);
        write_cpacr_el1(self.cpacr_el1);
        write_csselr_el1(self.csselr_el1);
        write_sp_el1(self.sp_el1);
        write_esr_el1(self.esr_el1);
        write_ttbr0_el1(self.ttbr0_el1);
        write_ttbr1_el1(self.ttbr1_el1);
        write_mair_el1(self.mair_el1);
        write_amair_el1(self.amair_el1);
        write_actlr_el1(self.actlr_el1);
        write_tpidr_el1(self.tpidr_el1);
        write_tpidr_el0(self.tpidr_el0);
        write_tpidrro_el0(self.tpidrro_el0);
        write_par_el1(self.par_el1);
        write_far_el1(self.far_el1);
        write_afsr0_el1(self.afsr0_el1);
        write_afsr1_el1(self.afsr1_el1);
        write_contextidr_el1(self.contextidr_el1);
        write_vbar_el1(self.vbar_el1);
        write_mdccint_el1(self.mdccint_el1);
        write_mdscr_el1(self.mdscr_el1);
    }
}

/// AArch64 EL2 system register context structure for preserving the architectural state during
/// world switches.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg(feature = "sel2")]
struct El2Sysregs {
    actlr_el2: u64,
    afsr0_el2: u64,
    afsr1_el2: u64,
    amair_el2: u64,
    cnthctl_el2: u64,
    cntvoff_el2: u64,
    contextidr_el2: u64,
    cptr_el2: u64,
    elr_el2: usize,
    esr_el2: Esr,
    far_el2: u64,
    hacr_el2: u64,
    hcr_el2: HcrEl2,
    hpfar_el2: u64,
    hstr_el2: u64,
    icc_sre_el2: IccSre,
    ich_hcr_el2: u64,
    ich_vmcr_el2: u64,
    mair_el2: u64,
    mdcr_el2: u64,
    sctlr_el2: u64,
    spsr_el2: Spsr,
    sp_el2: u64,
    tcr_el2: u64,
    tpidr_el2: u64,
    ttbr0_el2: u64,
    ttbr1_el2: u64,
    vbar_el2: usize,
    vmpidr_el2: u64,
    vpidr_el2: u64,
    vtcr_el2: u64,
    vttbr_el2: u64,
}

#[cfg(feature = "sel2")]
impl El2Sysregs {
    const EMPTY: Self = Self {
        actlr_el2: 0,
        afsr0_el2: 0,
        afsr1_el2: 0,
        amair_el2: 0,
        cnthctl_el2: 0,
        cntvoff_el2: 0,
        contextidr_el2: 0,
        cptr_el2: 0,
        elr_el2: 0,
        esr_el2: Esr::empty(),
        far_el2: 0,
        hacr_el2: 0,
        hcr_el2: HcrEl2::empty(),
        hpfar_el2: 0,
        hstr_el2: 0,
        icc_sre_el2: IccSre::empty(),
        ich_hcr_el2: 0,
        ich_vmcr_el2: 0,
        mair_el2: 0,
        mdcr_el2: 0,
        sctlr_el2: 0,
        spsr_el2: Spsr::empty(),
        sp_el2: 0,
        tcr_el2: 0,
        tpidr_el2: 0,
        ttbr0_el2: 0,
        ttbr1_el2: 0,
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

        if is_feat_vhe_present() {
            self.save_vhe();
        }
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

        if is_feat_vhe_present() {
            self.restore_vhe();
        }
    }

    fn save_vhe(&mut self) {
        self.contextidr_el2 = read_contextidr_el2();
        self.ttbr1_el2 = read_ttbr1_el2();
    }

    fn restore_vhe(&self) {
        write_contextidr_el2(self.contextidr_el2);
        write_ttbr1_el2(self.ttbr1_el2);
    }
}

/// Registers whose values can be shared across CPUs.
#[derive(Clone, Debug, Default)]
#[repr(C)]
pub struct PerWorldContext {
    cptr_el3: CptrEl3,
    zcr_el3: u64,
}

impl PerWorldContext {
    const EMPTY: Self = Self {
        cptr_el3: CptrEl3::empty(),
        zcr_el3: 0,
    };
}

pub type CrashBuf = [u64; CPU_DATA_CRASH_BUF_COUNT];

#[derive(Clone, Debug)]
#[repr(C, align(64))]
pub struct CpuData {
    cpu_ops_ptr: *const CpuOps,
    pub crash_buf: CrashBuf,
}

const _: () = assert!(size_of::<CpuData>() % align_of::<CpuData>() == 0);
const _: () = assert!(size_of::<CpuData>() % PlatformImpl::CACHE_WRITEBACK_GRANULE == 0);

impl CpuData {
    const EMPTY: Self = Self {
        cpu_ops_ptr: null(),
        crash_buf: [0; CPU_DATA_CRASH_BUF_COUNT],
    };
}

const CPU_MAX_PWR_DWN_OPS: usize = 2;

#[derive(Clone, Debug)]
#[repr(C)]
struct CpuOps {
    midr: u64,
    reset_func: *const extern "C" fn(),
    extra1_func: *const extern "C" fn(),
    extra2_func: *const extern "C" fn(),
    extra3_func: *const extern "C" fn(),
    e_handler_func: *const extern "C" fn(u64),
    pwr_dwn_ops: [*const extern "C" fn(); CPU_MAX_PWR_DWN_OPS],
    errata_list_start: *const u8,
    errata_list_end: *const u8,
    errata_func: *const u8,
    reg_dump: *const extern "C" fn(),
}

const _: () = assert!(size_of::<CpuOps>() % align_of::<CpuOps>() == 0);

pub static PER_WORLD_CONTEXT: PerWorld<PerWorldContext> =
    PerWorld([PerWorldContext::EMPTY; CPU_DATA_CONTEXT_NUM]);

#[unsafe(export_name = "percpu_data")]
static mut PERCPU_DATA: [CpuData; PlatformImpl::CORE_COUNT] =
    [CpuData::EMPTY; PlatformImpl::CORE_COUNT];

/// An array with one `T` for each world.
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct PerWorld<T>([T; CPU_DATA_CONTEXT_NUM]);

impl<T> Index<World> for PerWorld<T> {
    type Output = T;

    fn index(&self, world: World) -> &Self::Output {
        &self.0[world.index()]
    }
}

impl<T> IndexMut<World> for PerWorld<T> {
    fn index_mut(&mut self, world: World) -> &mut Self::Output {
        &mut self.0[world.index()]
    }
}

pub type CpuState = PerWorld<CpuContext>;

impl CpuState {
    const EMPTY: Self = Self([CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM]);
}

static CPU_STATE: PerCoreState<CpuState> = PerCore::new(
    [const { ExceptionLock::new(RefCell::new(CpuState::EMPTY)) }; PlatformImpl::CORE_COUNT],
);

/// Returns a raw pointer to the CPU context of the given world on the current core.
pub fn world_context(world: World) -> *mut CpuContext {
    // SAFETY: Getting the `CpuContext` pointer from a `CpuState` pointer requires the `CpuState`
    // pointer to be valid. We know that this is always true, because we get it from
    // `CPU_STATE.get().as_ptr()`. We avoid creating any intermediate references by accessing the
    // field of the `PerWorld` directly rather than using the `IndexMut` implementation.
    unsafe { &raw mut (*CPU_STATE.get().as_ptr()).0[world.index()] }
}

/// Saves lower EL system registers from the current world, restores lower EL system registers of
/// the given world.
pub fn switch_world(old_world: World, new_world: World) {
    assert_ne!(old_world, new_world);
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        cpu_state[old_world].save_lower_el_sysregs();
        cpu_state[new_world].restore_lower_el_sysregs();
    });
}

/// Restores lower EL system registers of the given world.
///
/// This doesn't save the current state of the lower EL system registers, so should only be used for
/// initial boot where we don't care about their state.
pub fn set_initial_world(world: World) {
    exception_free(|token| {
        let cpu_state = cpu_state(token);
        let context = &cpu_state[world];

        // This must be initialised before the EL2 system registers are written to, to avoid an
        // exception.
        write_scr_el3(context.el3_state.scr_el3);
        isb();

        context.restore_lower_el_sysregs();
    });
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
        initialise_nonsecure(&mut cpu_state[World::NonSecure], non_secure_entry_point);
        initialise_secure(&mut cpu_state[World::Secure], secure_entry_point);
        #[cfg(feature = "rme")]
        initialise_realm(&mut cpu_state[World::Realm], realm_entry_point);
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
    // against ERRATA_V2_3099206.
    context.el3_state.scr_el3 = ScrEl3::RES1 | ScrEl3::HCE | ScrEl3::EA | ScrEl3::SIF | ScrEl3::RW;
    #[cfg(feature = "sel2")]
    {
        context.el3_state.scr_el3 |= ScrEl3::EEL2;
        // TODO: Initialise the rest of the context.el2_sysregs too.
        context.el2_sysregs.icc_sre_el2 = IccSre::DIB | IccSre::DFB | IccSre::EN | IccSre::SRE;
    }
    #[cfg(not(feature = "sel2"))]
    {
        context.el1_sysregs.sctlr_el1 = SctlrEl1::RES1;
    }
}

/// Initialises the given CPU context ready for booting NS-EL2 or NS-EL1.
fn initialise_nonsecure(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);
    context.el3_state.scr_el3 |= ScrEl3::NS;

    gicv3::set_routing_model(&mut context.el3_state.scr_el3, World::NonSecure);
}

/// Initialises the given CPU context ready for booting S-EL2 or S-EL1.
fn initialise_secure(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);

    // Enable Secure EL1 access to timer registers.
    // Otherwise they would be accessible only at EL3.
    context.el3_state.scr_el3 |= ScrEl3::ST;

    gicv3::set_routing_model(&mut context.el3_state.scr_el3, World::Secure);
}

/// Initialises the given CPU context ready for booting Realm world
#[cfg(feature = "rme")]
fn initialise_realm(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    initialise_common(context, entry_point);
    // SCR_NS + SCR_NSE = Realm state
    context.el3_state.scr_el3 |= ScrEl3::NS | ScrEl3::NSE;
    // TODO: FIQ and IRQ routing model.
}

/// Updates the CPU context of each world to resume after suspend.
///
/// When the CPU wakes up from a powerdown suspend state, lower ELs in each world expect a specific
/// state for resuming their execution. This can be a different entry point or just arguments passed
/// in registers.
pub fn update_contexts_suspend(psci_entrypoint: EntryPoint, secure_args: &SmcReturn) {
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);

        cpu_state[World::NonSecure].el3_state.elr_el3 =
            psci_entrypoint.entry_point_address() as usize;
        cpu_state[World::NonSecure].gpregs.registers[0] = psci_entrypoint.context_id();
        cpu_state[World::NonSecure].gpregs.registers[1..8].fill(0);

        cpu_state[World::Secure].gpregs.registers[..18].copy_from_slice(secure_args.values());

        // TODO: implement suspend handling for Realm
    });
}

/// Information about the entry point for a next stage (e.g. BL32 or BL33).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryPointInfo {
    /// The entry point address.
    pub pc: usize,
    /// The `spsr_el3` value to set before `eret`, to set the appropriate PSTATE.
    pub spsr: Spsr,
    /// Boot arguments to pass in `x0`-`x7`.
    pub args: [u64; 8],
}

#[cfg(all(target_arch = "aarch64", not(test)))]
#[allow(clippy::manual_bits)]
mod asm {
    use super::*;
    use crate::{
        debug::{CRASH_REPORTING, DEBUG, ENABLE_ASSERTIONS},
        exceptions::RunResult,
        smccc::NOT_SUPPORTED,
    };
    use arm_sysregs::{Pmcr, StackPointer};
    use core::{
        arch::global_asm,
        mem::{offset_of, size_of},
    };

    // TODO: Let this be controlled by the platform or a cargo feature.
    const ERRATA_SPECULATIVE_AT: bool = false;

    #[cfg(not(feature = "sel2"))]
    const CTX_EL1_SYSREGS_OFFSET: usize = offset_of!(CpuContext, el1_sysregs);
    #[cfg(not(feature = "sel2"))]
    const CTX_SCTLR_EL1: usize = offset_of!(El1Sysregs, sctlr_el1);

    // These are not actually used because we don't support ERRATA_SPECULATIVE_AT and S-EL2 together,
    // but we still need to define some values to substitute into context.S.
    #[cfg(feature = "sel2")]
    const CTX_EL1_SYSREGS_OFFSET: usize = 0;
    #[cfg(feature = "sel2")]
    const CTX_SCTLR_EL1: usize = 0;

    // ERRATA_SPECULATIVE_AT requires El1Sysregs.
    #[cfg(feature = "sel2")]
    const _: () = assert!(!ERRATA_SPECULATIVE_AT);

    const MIDR_IMPL_MASK: u32 = 0xff;
    const MIDR_IMPL_SHIFT: u8 = 0x18;
    const MIDR_PN_MASK: u32 = 0xfff;
    const MIDR_PN_SHIFT: u8 = 0x4;
    const CPU_IMPL_PN_MASK: u32 =
        (MIDR_IMPL_MASK << MIDR_IMPL_SHIFT) | (MIDR_PN_MASK << MIDR_PN_SHIFT);

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("context.S"),
        include_str!("runtime_exceptions.S"),
        include_str!("cpu_helpers.S"),
        include_str!("cpu_data.S"),
        include_str!("asm_macros_common_purge.S"),
        ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
        DEBUG = const DEBUG as u32,
        ERRATA_SPECULATIVE_AT = const ERRATA_SPECULATIVE_AT as u32,
        SCR_EA_BIT = const ScrEl3::EA.bits(),
        PMCR_EL0_DP_BIT = const Pmcr::DP.bits(),
        MODE_SP_EL0 = const StackPointer::El0 as u8,
        MODE_SP_ELX = const StackPointer::ElX as u8,
        CPTR_EZ_BIT = const CptrEl3::EZ.bits(),
        SCR_NSE_SHIFT = const 62,
        CTX_NESTED_EA_FLAG = const offset_of!(El3State, nested_ea_flag),
        CTX_GPREGS_OFFSET = const offset_of!(GpRegs, registers),
        CTX_EL3STATE_OFFSET = const offset_of!(CpuContext, el3_state),
        CTX_EL1_SYSREGS_OFFSET = const CTX_EL1_SYSREGS_OFFSET,
        CTX_SCTLR_EL1 = const CTX_SCTLR_EL1,
        CTX_PMCR_EL0 = const offset_of!(El3State, pmcr_el0),
        CTX_SCR_EL3 = const offset_of!(El3State, scr_el3),
        CTX_SPSR_EL3 = const offset_of!(El3State, spsr_el3),
        CTX_RUNTIME_SP_LR = const offset_of!(El3State, runtime_sp),
        CTX_CPTR_EL3 = const offset_of!(PerWorldContext, cptr_el3),
        CTX_SAVED_ELR_EL3 = const offset_of!(El3State, saved_elr_el3),
        CTX_GPREG_X0 = const 0,
        CTX_GPREG_X2 = const 2 * size_of::<u64>(),
        CTX_GPREG_X4 = const 4 * size_of::<u64>(),
        CTX_GPREG_X6 = const 6 * size_of::<u64>(),
        CTX_GPREG_X8 = const 8 * size_of::<u64>(),
        CTX_GPREG_X10 = const 10 * size_of::<u64>(),
        CTX_GPREG_X12 = const 12 * size_of::<u64>(),
        CTX_GPREG_X14 = const 14 * size_of::<u64>(),
        CTX_GPREG_X16 = const 16 * size_of::<u64>(),
        CTX_GPREG_X18 = const 18 * size_of::<u64>(),
        CTX_GPREG_X20 = const 20 * size_of::<u64>(),
        CTX_GPREG_X22 = const 22 * size_of::<u64>(),
        CTX_GPREG_X24 = const 24 * size_of::<u64>(),
        CTX_GPREG_X26 = const 26 * size_of::<u64>(),
        CTX_GPREG_X28 = const 28 * size_of::<u64>(),
        CTX_GPREG_X29 = const 29 * size_of::<u64>(),
        CTX_GPREG_LR = const 30 * size_of::<u64>(),
        CTX_GPREG_SP_EL0 = const 31 * size_of::<u64>(),
        ISR_A_SHIFT = const 8,
        SMC_UNK = const NOT_SUPPORTED,
        CPU_E_HANDLER_FUNC = const offset_of!(CpuOps, e_handler_func),
        RUN_RESULT_SMC = const RunResult::SMC,
        RUN_RESULT_SYSREG_TRAP = const RunResult::SYSREG_TRAP,
        RUN_RESULT_INTERRUPT = const RunResult::INTERRUPT,
        CPU_DATA_CPU_OPS_PTR = const offset_of!(CpuData, cpu_ops_ptr),
        CPU_MIDR = const offset_of!(CpuOps, midr),
        CPU_REG_DUMP = const offset_of!(CpuOps, reg_dump),
        CPU_OPS_SIZE = const size_of::<CpuOps>(),
        CPU_IMPL_PN_MASK = const CPU_IMPL_PN_MASK,
        CRASH_REPORTING = const CRASH_REPORTING as u32,
        SUPPORT_UNKNOWN_MPID = const 0,
        CPU_DATA_SIZE = const size_of::<CpuData>(),
    );
}
