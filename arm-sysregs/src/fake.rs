// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake implementations of system register getters and setters for unit tests.

use super::{
    ClidrEl1, CptrEl3, CsselrEl1, CtrEl0, Esr, Gcscr, HcrEl2, HcrxEl2, IccSre, IdAa64dfr0El1,
    IdAa64dfr1El1, IdAa64isar1El1, IdAa64isar2El1, IdAa64mmfr0El1, IdAa64mmfr1El1, IdAa64mmfr2El1,
    IdAa64mmfr3El1, IdAa64pfr0El1, IdAa64pfr1El1, IdAa64smfr0El1, MdcrEl2, MidrEl1, Mpam3El3,
    MpamIdrEl1, MpidrEl1, Pmcr, ScrEl3, Sctlr2El3, SctlrEl1, SctlrEl2, SctlrEl3, SmcrEl3, Spsr,
};
use std::sync::Mutex;

/// Generates a public function named `read_$sysreg` to read the fake system register `$sysreg` of
/// type `$type`.
#[macro_export]
macro_rules! read_sysreg {
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< read_ $sysreg >]() -> $type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< read_ $sysreg >]() -> $type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty : $bitflags_type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< read_ $sysreg >]() -> $bitflags_type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty : $bitflags_type:ty, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< read_ $sysreg >]() -> $bitflags_type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
}

/// Generates a public function named `write_$sysreg` to write to the fake system register `$sysreg`
/// of type `$type`.
#[macro_export]
macro_rules! write_sysreg {
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< write_ $sysreg >](value: $type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty, $fake_sysregs:expr
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< write_ $sysreg >](value: $type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty : $bitflags_type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< write_ $sysreg >](value: $bitflags_type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty : $bitflags_type:ty, $fake_sysregs:expr
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< write_ $sysreg >](value: $bitflags_type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
}

/// Values of fake system registers.
pub static SYSREGS: Mutex<SystemRegisters> = Mutex::new(SystemRegisters::new());

/// A set of fake system registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRegisters {
    /// Fake value for the ACTLR_EL1 system register.
    pub actlr_el1: u64,
    /// Fake value for the ACTLR_EL2 system register.
    pub actlr_el2: u64,
    /// Fake value for the AFSR0_EL1 system register.
    pub afsr0_el1: u64,
    /// Fake value for the AFSR0_EL2 system register.
    pub afsr0_el2: u64,
    /// Fake value for the AFSR1_EL1 system register.
    pub afsr1_el1: u64,
    /// Fake value for the AFSR1_EL2 system register.
    pub afsr1_el2: u64,
    /// Fake value for the AMAIR_EL1 system register.
    pub amair_el1: u64,
    /// Fake value for the AMAIR_EL2 system register.
    pub amair_el2: u64,
    /// Fake value for the APIAKEYLO_EL1 system register.
    pub apiakeylo_el1: u64,
    /// Fake value for the APIAKEYHI_EL1 system register.
    pub apiakeyhi_el1: u64,
    /// Fake value for the CCSIDR_EL1 system register.
    pub ccsidr_el1: u64,
    /// Fake value for the CLIDR_EL1 system register.
    pub clidr_el1: ClidrEl1,
    /// Fake value for the CNTFRQ_EL0 system register.
    pub cntfrq_el0: u64,
    /// Fake value for the CNTHCTL_EL2 system register.
    pub cnthctl_el2: u64,
    /// Fake value for the CNTPCT_EL0 system register.
    pub cntpct_el0: u64,
    /// Fake value for the CNTVOFF_EL2 system register.
    pub cntvoff_el2: u64,
    /// Fake value for the CONTEXTIDR_EL1 system register.
    pub contextidr_el1: u64,
    /// Fake value for the CONTEXTIDR_EL2 system register.
    pub contextidr_el2: u64,
    /// Fake value for the CPACR_EL1 system register.
    pub cpacr_el1: u64,
    /// Fake value for the CPTR_EL2 system register.
    pub cptr_el2: u64,
    /// Fake value for the CPTR_EL3 system register.
    pub cptr_el3: CptrEl3,
    /// Fake value for the CSSELR_EL1 system register.
    pub csselr_el1: CsselrEl1,
    /// Fake value for the CTR_EL0 system register.
    pub ctr_el0: CtrEl0,
    /// Fake value for the DISR_EL1 system register.
    pub disr_el1: u64,
    /// Fake value for the ELR_EL1 system register.
    pub elr_el1: usize,
    /// Fake value for the ELR_EL2 system register.
    pub elr_el2: usize,
    /// Fake value for the ESR_EL1 system register.
    pub esr_el1: Esr,
    /// Fake value for the ESR_EL2 system register.
    pub esr_el2: Esr,
    /// Fake value for the FAR_EL1 system register.
    pub far_el1: u64,
    /// Fake value for the FAR_EL2 system register.
    pub far_el2: u64,
    /// Fake value for the GCR_EL1 system register.
    pub gcr_el1: u64,
    /// Fake value for the GCSCR_EL1 system register.
    pub gcscr_el1: Gcscr,
    /// Fake value for the GCSCR_EL2 system register.
    pub gcscr_el2: Gcscr,
    /// Fake value for the HACR_EL2 system register.
    pub hacr_el2: u64,
    /// Fake value for the HCR_EL2 system register.
    pub hcr_el2: HcrEl2,
    /// Fake value for the HCRX_EL2 system register.
    pub hcrx_el2: HcrxEl2,
    /// Fake value for the HDFGRTR2_EL2 system register.
    pub hdfgrtr2_el2: u64,
    /// Fake value for the HDFGWTR2_EL2 system register.
    pub hdfgwtr2_el2: u64,
    /// Fake value for the HFGITR2_EL2 system register.
    pub hfgitr2_el2: u64,
    /// Fake value for the HFGRTR2_EL2 system register.
    pub hfgrtr2_el2: u64,
    /// Fake value for the HFGWTR2_EL2 system register.
    pub hfgwtr2_el2: u64,
    /// Fake value for the HPFAR_EL2 system register.
    pub hpfar_el2: u64,
    /// Fake value for the HSTR_EL2 system register.
    pub hstr_el2: u64,
    /// Fake value for the ICC_SRE_EL1 system register.
    pub icc_sre_el1: IccSre,
    /// Fake value for the ICC_SRE_EL2 system register.
    pub icc_sre_el2: IccSre,
    /// Fake value for the ICC_SRE_EL3 system register.
    pub icc_sre_el3: IccSre,
    /// Fake value for the ICH_HCR_EL2 system register.
    pub ich_hcr_el2: u64,
    /// Fake value for the ICH_VMCR_EL2 system register.
    pub ich_vmcr_el2: u64,
    /// Fake value for the ID_AA64DFR0_EL1 system register.
    pub id_aa64dfr0_el1: IdAa64dfr0El1,
    /// Fake value for the ID_AA64DFR1_EL1 system register.
    pub id_aa64dfr1_el1: IdAa64dfr1El1,
    /// Fake value for the ID_AA64ISAR1_EL1 system register.
    pub id_aa64isar1_el1: IdAa64isar1El1,
    /// Fake value for the ID_AA64ISAR2_EL1 system register.
    pub id_aa64isar2_el1: IdAa64isar2El1,
    /// Fake value for the ID_AA64MMFR0_EL1 system register.
    pub id_aa64mmfr0_el1: IdAa64mmfr0El1,
    /// Fake value for the ID_AA64MMFR1_EL1 system register.
    pub id_aa64mmfr1_el1: IdAa64mmfr1El1,
    /// Fake value for the ID_AA64MMFR2_EL1 system register.
    pub id_aa64mmfr2_el1: IdAa64mmfr2El1,
    /// Fake value for the ID_AA64MMFR3_EL1 system register.
    pub id_aa64mmfr3_el1: IdAa64mmfr3El1,
    /// Fake value for the ID_AA64PFR0_EL1 system register.
    pub id_aa64pfr0_el1: IdAa64pfr0El1,
    /// Fake value for the ID_AA64PFR1_EL1 system register.
    pub id_aa64pfr1_el1: IdAa64pfr1El1,
    /// Fake value for the ID_AA64SMFR0_EL1 system register.
    pub id_aa64smfr0_el1: IdAa64smfr0El1,
    /// Fake value for the ISR_EL1 system register.
    pub isr_el1: u64,
    /// Fake value for the MAIR_EL1 system register.
    pub mair_el1: u64,
    /// Fake value for the MAIR_EL2 system register.
    pub mair_el2: u64,
    /// Fake value for the MAIR_EL3 system register.
    pub mair_el3: u64,
    /// Fake value for the MDCCINT_EL1 system register.
    pub mdccint_el1: u64,
    /// Fake value for the MDCR_EL2 system register.
    pub mdcr_el2: MdcrEl2,
    /// Fake value for the MDSCR_EL1 system register.
    pub mdscr_el1: u64,
    /// Fake value for the MIDR_EL1 system register.
    pub midr_el1: MidrEl1,
    /// Fake value for the MPAM2_EL2 system register.
    pub mpam2_el2: u64,
    /// Fake value for the MPAM3_EL3 system register.
    pub mpam3_el3: Mpam3El3,
    /// Fake value for the MPAMHCR_EL2 system register.
    pub mpamhcr_el2: u64,
    /// Fake value for the MPAMIDR_EL1 system register.
    pub mpamidr_el1: MpamIdrEl1,
    /// Fake value for the MPAMVPMV_EL2 system register.
    pub mpamvpmv_el2: u64,
    /// Fake value for the MPAMVPM0_EL2 system register.
    pub mpamvpm0_el2: u64,
    /// Fake value for the MPAMVPM1_EL2 system register.
    pub mpamvpm1_el2: u64,
    /// Fake value for the MPAMVPM2_EL2 system register.
    pub mpamvpm2_el2: u64,
    /// Fake value for the MPAMVPM3_EL2 system register.
    pub mpamvpm3_el2: u64,
    /// Fake value for the MPAMVPM4_EL2 system register.
    pub mpamvpm4_el2: u64,
    /// Fake value for the MPAMVPM5_EL2 system register.
    pub mpamvpm5_el2: u64,
    /// Fake value for the MPAMVPM6_EL2 system register.
    pub mpamvpm6_el2: u64,
    /// Fake value for the MPAMVPM7_EL2 system register.
    pub mpamvpm7_el2: u64,
    /// Fake value for the MPIDR_EL1 system register.
    pub mpidr_el1: MpidrEl1,
    /// Fake value for THEPAR_EL1 system register.
    pub par_el1: u64,
    /// Fake value for PMCR_EL0 system register.
    pub pmcr_el0: Pmcr,
    /// Fake value for the RGSR_EL1 system register.
    pub rgsr_el1: u64,
    /// Fake value for THESCR_EL3 system register.
    pub scr_el3: ScrEl3,
    /// Fake value for THESCTLR_EL1 system register.
    pub sctlr_el1: SctlrEl1,
    /// Fake value for THESCTLR_EL2 system register.
    pub sctlr_el2: SctlrEl2,
    /// Fake value for THESCTLR_EL3 system register.
    pub sctlr_el3: SctlrEl3,
    /// Fake value for the SCTLR2_EL3 system register.
    pub sctlr2_el3: Sctlr2El3,
    /// Fake value for SMCR_EL3 system register.
    pub smcr_el3: SmcrEl3,
    /// Fake value for THESP_EL1 system register.
    pub sp_el1: u64,
    /// Fake value for THESP_EL2 system register.
    pub sp_el2: u64,
    /// Fake value for THESP_EL3 system register.
    pub sp_el3: usize,
    /// Fake value for THESPSR_EL1 system register.
    pub spsr_el1: Spsr,
    /// Fake value for THESPSR_EL2 system register.
    pub spsr_el2: Spsr,
    /// Fake value for THETCR_EL1 system register.
    pub tcr_el1: u64,
    /// Fake value for THETCR_EL2 system register.
    pub tcr_el2: u64,
    /// Fake value for THETCR_EL3 system register.
    pub tcr_el3: u64,
    /// Fake value for the TCR2_EL1 system register.
    pub tcr2_el1: u64,
    /// Fake value for the TCR2_EL2 system register.
    pub tcr2_el2: u64,
    /// Fake value for the TFSR_EL1 system register.
    pub tfsr_el1: u64,
    /// Fake value for the TFSR_EL2 system register.
    pub tfsr_el2: u64,
    /// Fake value for the TFSRE0_EL1 system register.
    pub tfsre0_el1: u64,
    /// Fake value for THETPIDR_EL0 system register.
    pub tpidr_el0: u64,
    /// Fake value for THETPIDR_EL1 system register.
    pub tpidr_el1: u64,
    /// Fake value for THETPIDR_EL2 system register.
    pub tpidr_el2: u64,
    /// Fake value for THETPIDRRO_EL0 system register.
    pub tpidrro_el0: u64,
    /// Fake value for THETTBR0_EL1 system register.
    pub ttbr0_el1: u64,
    /// Fake value for THETTBR0_EL2 system register.
    pub ttbr0_el2: u64,
    /// Fake value for THETTBR0_EL3 system register.
    pub ttbr0_el3: usize,
    /// Fake value for THETTBR1_EL1 system register.
    pub ttbr1_el1: u64,
    /// Fake value for THETTBR1_EL2 system register.
    pub ttbr1_el2: u64,
    /// Fake value for THEVBAR_EL1 system register.
    pub vbar_el1: usize,
    /// Fake value for THEVBAR_EL2 system register.
    pub vbar_el2: usize,
    /// Fake value for the VDISR_EL2 system register.
    pub vdisr_el2: u64,
    /// Fake value for THEVMPIDR_EL2 system register.
    pub vmpidr_el2: u64,
    /// Fake value for THEVPIDR_EL2 system register.
    pub vpidr_el2: u64,
    /// Fake value for the VSESR_EL2 system register.
    pub vsesr_el2: u64,
    /// Fake value for THEVTCR_EL2 system register.
    pub vtcr_el2: u64,
    /// Fake value for THEVTTBR_EL2 system register.
    pub vttbr_el2: u64,
    /// Fake value for ZCR_EL3 system register.
    pub zcr_el3: u64,
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
            apiakeylo_el1: 0,
            apiakeyhi_el1: 0,
            ccsidr_el1: 0,
            clidr_el1: ClidrEl1::empty(),
            cntfrq_el0: 0,
            cnthctl_el2: 0,
            cntpct_el0: 0,
            cntvoff_el2: 0,
            contextidr_el1: 0,
            contextidr_el2: 0,
            cpacr_el1: 0,
            cptr_el2: 0,
            cptr_el3: CptrEl3::empty(),
            csselr_el1: CsselrEl1::empty(),
            ctr_el0: CtrEl0::empty(),
            disr_el1: 0,
            elr_el1: 0,
            elr_el2: 0,
            esr_el1: Esr::empty(),
            esr_el2: Esr::empty(),
            far_el1: 0,
            far_el2: 0,
            gcr_el1: 0,
            gcscr_el1: Gcscr::empty(),
            gcscr_el2: Gcscr::empty(),
            hacr_el2: 0,
            hcr_el2: HcrEl2::empty(),
            hcrx_el2: HcrxEl2::empty(),
            hdfgrtr2_el2: 0,
            hdfgwtr2_el2: 0,
            hfgitr2_el2: 0,
            hfgrtr2_el2: 0,
            hfgwtr2_el2: 0,
            hpfar_el2: 0,
            hstr_el2: 0,
            icc_sre_el1: IccSre::empty(),
            icc_sre_el2: IccSre::empty(),
            icc_sre_el3: IccSre::empty(),
            ich_hcr_el2: 0,
            ich_vmcr_el2: 0,
            id_aa64dfr0_el1: IdAa64dfr0El1::empty(),
            id_aa64dfr1_el1: IdAa64dfr1El1::empty(),
            id_aa64isar1_el1: IdAa64isar1El1::empty(),
            id_aa64isar2_el1: IdAa64isar2El1::empty(),
            id_aa64mmfr0_el1: IdAa64mmfr0El1::empty(),
            id_aa64mmfr1_el1: IdAa64mmfr1El1::empty(),
            id_aa64mmfr2_el1: IdAa64mmfr2El1::empty(),
            id_aa64mmfr3_el1: IdAa64mmfr3El1::empty(),
            id_aa64pfr0_el1: IdAa64pfr0El1::empty(),
            id_aa64pfr1_el1: IdAa64pfr1El1::empty(),
            id_aa64smfr0_el1: IdAa64smfr0El1::empty(),
            isr_el1: 0,
            mair_el1: 0,
            mair_el2: 0,
            mair_el3: 0,
            mdccint_el1: 0,
            mdcr_el2: MdcrEl2::empty(),
            mdscr_el1: 0,
            midr_el1: MidrEl1::empty(),
            mpam2_el2: 0,
            mpam3_el3: Mpam3El3::empty(),
            mpamhcr_el2: 0,
            mpamidr_el1: MpamIdrEl1::empty(),
            mpamvpmv_el2: 0,
            mpamvpm0_el2: 0,
            mpamvpm1_el2: 0,
            mpamvpm2_el2: 0,
            mpamvpm3_el2: 0,
            mpamvpm4_el2: 0,
            mpamvpm5_el2: 0,
            mpamvpm6_el2: 0,
            mpamvpm7_el2: 0,
            mpidr_el1: MpidrEl1::empty(),
            par_el1: 0,
            pmcr_el0: Pmcr::empty(),
            rgsr_el1: 0,
            scr_el3: ScrEl3::empty(),
            sctlr_el1: SctlrEl1::empty(),
            sctlr_el2: SctlrEl2::empty(),
            sctlr_el3: SctlrEl3::empty(),
            sctlr2_el3: Sctlr2El3::empty(),
            smcr_el3: SmcrEl3::empty(),
            sp_el1: 0,
            sp_el2: 0,
            sp_el3: 0,
            spsr_el1: Spsr::empty(),
            spsr_el2: Spsr::empty(),
            tcr_el1: 0,
            tcr_el2: 0,
            tcr_el3: 0,
            tcr2_el1: 0,
            tcr2_el2: 0,
            tfsr_el1: 0,
            tfsr_el2: 0,
            tfsre0_el1: 0,
            tpidr_el0: 0,
            tpidr_el1: 0,
            tpidr_el2: 0,
            tpidrro_el0: 0,
            ttbr0_el1: 0,
            ttbr0_el2: 0,
            ttbr0_el3: 0,
            ttbr1_el1: 0,
            ttbr1_el2: 0,
            vbar_el1: 0,
            vbar_el2: 0,
            vdisr_el2: 0,
            vmpidr_el2: 0,
            vpidr_el2: 0,
            vsesr_el2: 0,
            vtcr_el2: 0,
            vttbr_el2: 0,
            zcr_el3: 0,
        }
    }

    /// Resets the fake system registers to their initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
