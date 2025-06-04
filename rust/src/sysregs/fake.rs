// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake implementations of system register getters and setters for unit tests.

use super::{Esr, HcrEl2, IccSre, MpidrEl1, ScrEl3, SctlrEl1, SctlrEl3, Spsr};
use std::sync::Mutex;

/// Values of fake system registers.
pub static SYSREGS: Mutex<SystemRegisters> = Mutex::new(SystemRegisters::new());

/// A set of fake system registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRegisters {
    pub actlr_el1: u64,
    pub actlr_el2: u64,
    pub afsr0_el1: u64,
    pub afsr0_el2: u64,
    pub afsr1_el1: u64,
    pub afsr1_el2: u64,
    pub amair_el1: u64,
    pub amair_el2: u64,
    pub cnthctl_el2: u64,
    pub cntvoff_el2: u64,
    pub contextidr_el1: u64,
    pub cpacr_el1: u64,
    pub cptr_el2: u64,
    pub csselr_el1: u64,
    pub elr_el1: usize,
    pub elr_el2: usize,
    pub esr_el1: Esr,
    pub esr_el2: Esr,
    pub far_el1: u64,
    pub far_el2: u64,
    pub hacr_el2: u64,
    pub hcr_el2: HcrEl2,
    pub hpfar_el2: u64,
    pub hstr_el2: u64,
    pub icc_sre_el1: IccSre,
    pub icc_sre_el2: IccSre,
    pub icc_sre_el3: IccSre,
    pub ich_hcr_el2: u64,
    pub ich_vmcr_el2: u64,
    pub id_aa64mmfr1_el1: u64,
    pub isr_el1: u64,
    pub mair_el1: u64,
    pub mair_el2: u64,
    pub mair_el3: u64,
    pub mdccint_el1: u64,
    pub mdcr_el2: u64,
    pub mdscr_el1: u64,
    pub mpidr_el1: MpidrEl1,
    pub par_el1: u64,
    pub scr_el3: ScrEl3,
    pub sctlr_el1: SctlrEl1,
    pub sctlr_el2: u64,
    pub sctlr_el3: SctlrEl3,
    pub sp_el1: u64,
    pub sp_el2: u64,
    pub sp_el3: usize,
    pub spsr_el1: Spsr,
    pub spsr_el2: Spsr,
    pub tcr_el1: u64,
    pub tcr_el2: u64,
    pub tcr_el3: u64,
    pub tpidr_el0: u64,
    pub tpidr_el1: u64,
    pub tpidr_el2: u64,
    pub tpidrro_el0: u64,
    pub ttbr0_el1: u64,
    pub ttbr0_el2: u64,
    pub ttbr0_el3: usize,
    pub ttbr1_el1: u64,
    pub vbar_el1: usize,
    pub vbar_el2: usize,
    pub vmpidr_el2: u64,
    pub vpidr_el2: u64,
    pub vtcr_el2: u64,
    pub vttbr_el2: u64,
}

impl SystemRegisters {
    const fn new() -> Self {
        Self {
            actlr_el1: 0,
            actlr_el2: 0,
            afsr0_el1: 0,
            afsr0_el2: 0,
            afsr1_el1: 0,
            afsr1_el2: 0,
            amair_el1: 0,
            amair_el2: 0,
            cnthctl_el2: 0,
            cntvoff_el2: 0,
            contextidr_el1: 0,
            cpacr_el1: 0,
            cptr_el2: 0,
            csselr_el1: 0,
            elr_el1: 0,
            elr_el2: 0,
            esr_el1: Esr::empty(),
            esr_el2: Esr::empty(),
            far_el1: 0,
            far_el2: 0,
            hacr_el2: 0,
            hcr_el2: HcrEl2::empty(),
            hpfar_el2: 0,
            hstr_el2: 0,
            icc_sre_el1: IccSre::empty(),
            icc_sre_el2: IccSre::empty(),
            icc_sre_el3: IccSre::empty(),
            ich_hcr_el2: 0,
            ich_vmcr_el2: 0,
            id_aa64mmfr1_el1: 0,
            isr_el1: 0,
            mair_el1: 0,
            mair_el2: 0,
            mair_el3: 0,
            mdccint_el1: 0,
            mdcr_el2: 0,
            mdscr_el1: 0,
            mpidr_el1: MpidrEl1::empty(),
            par_el1: 0,
            scr_el3: ScrEl3::empty(),
            sctlr_el1: SctlrEl1::empty(),
            sctlr_el2: 0,
            sctlr_el3: SctlrEl3::empty(),
            sp_el1: 0,
            sp_el2: 0,
            sp_el3: 0,
            spsr_el1: Spsr::empty(),
            spsr_el2: Spsr::empty(),
            tcr_el1: 0,
            tcr_el2: 0,
            tcr_el3: 0,
            tpidr_el0: 0,
            tpidr_el1: 0,
            tpidr_el2: 0,
            tpidrro_el0: 0,
            ttbr0_el1: 0,
            ttbr0_el2: 0,
            ttbr0_el3: 0,
            ttbr1_el1: 0,
            vbar_el1: 0,
            vbar_el2: 0,
            vmpidr_el2: 0,
            vpidr_el2: 0,
            vtcr_el2: 0,
            vttbr_el2: 0,
        }
    }

    /// Reset the fake system registers to their initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// Writes `value` to `sp_el3`.
///
/// # Safety
///
/// The caller must ensure that `value` is consistent with how the rest of RF-A uses `sp_el3`.
pub unsafe fn write_sp_el3(value: usize) {
    let mut regs = SYSREGS.lock().unwrap();
    regs.sp_el3 = value;
}

/// Generates a public function named `$function_name` to read the fake system register `$sysreg` of
/// type `$type`.
macro_rules! read_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg
        }
    };
    ($sysreg:ident, $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg
        }
    };
}

/// Generates a public function named `$function_name` to write to the fake system register
/// `$sysreg` of type `$type`.
macro_rules! write_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident) => {
        pub fn $function_name(value: $type) {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg = value;
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty, $function_name:ident
    ) => {
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $type) {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg = value;
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $function_name:ident) => {
        pub fn $function_name(value: $type) {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg = value;
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $raw_type:ty : $type:ty, $function_name:ident
    ) => {
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $type) {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg = value;
        }
    };
}
