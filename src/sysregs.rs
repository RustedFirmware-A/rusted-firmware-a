// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
#[macro_use]
pub mod fake;

use arm_psci::Mpidr;
use bitflags::bitflags;
#[cfg(not(test))]
use core::arch::asm;
use core::fmt::{self, Debug, Formatter};

/// Constants for PMCR_EL0 fields.
pub mod pmcr {
    /// Disable cycle counter when event counting is prohibited.
    pub const DP: u64 = 1 << 5;
}

/// Implements a similar interface to `bitflags` on some newtype.
macro_rules! bitflagslike {
    ($typename:ty: $inner:ty) => {
        impl $typename {
            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn bits(self) -> $inner {
                self.0
            }

            pub const fn from_bits_retain(bits: $inner) -> Self {
                Self(bits)
            }
        }

        impl core::ops::Not for $typename {
            type Output = Self;

            fn not(self) -> Self {
                Self(!self.0)
            }
        }

        impl core::ops::BitOr for $typename {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitOrAssign for $typename {
            fn bitor_assign(&mut self, rhs: Self) {
                *self = *self | rhs
            }
        }

        impl core::ops::BitAnd for $typename {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::BitAndAssign for $typename {
            fn bitand_assign(&mut self, rhs: Self) {
                *self = *self & rhs
            }
        }
    };
}

/// Generates a public function named `$function_name` to read the system register `$sysreg` as a
/// value of type `$type`.
///
/// `safe` should only be specified for system registers which are indeed safe to read.
#[cfg(not(test))]
macro_rules! read_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            let value;
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is
            // always safe to read the given `$sysreg.`
            unsafe {
                asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            value
        }
    };
    ($sysreg:ident, $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            let value;
            // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
            unsafe {
                asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            value
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            let value: $raw_type;
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is
            // always safe to read the given `$sysreg.`
            unsafe {
                asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            <$type>::from_bits_retain(value)
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            let value: $raw_type;
            // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
            unsafe {
                asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            <$type>::from_bits_retain(value)
        }
    };
}

/// Generates a public function named `$function_name` to write a value of type `$type` to the
/// system register `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to write any value
/// to.
#[cfg(not(test))]
macro_rules! write_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident) => {
        pub fn $function_name(value: $type) {
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
            // to write any value to the given `$sysreg.`
            unsafe {
                asm!(
                    concat!("msr ", stringify!($sysreg), ", {value}"),
                    options(nostack),
                    value = in(reg) value,
                );
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty, $function_name:ident
    ) => {
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $type) {
            // SAFETY: The caller promises that it is safe to write `value` to the given `$sysreg`.
            unsafe {
                asm!(
                    concat!("msr ", stringify!($sysreg), ", {value}"),
                    options(nostack),
                    value = in(reg) value,
                );
            }
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $function_name:ident) => {
        pub fn $function_name(value: $type) {
            let value: $raw_type = value.bits();
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
            // to write any value to the given `$sysreg.`
            unsafe {
                asm!(
                    concat!("msr ", stringify!($sysreg), ", {value}"),
                    options(nostack),
                    value = in(reg) value,
                );
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $raw_type:ty : $type:ty, $function_name:ident
    ) => {
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $type) {
            let value: $raw_type = value.bits();
            // SAFETY: The caller promises that it is safe to write `value` to the given `$sysreg`.
            unsafe {
                asm!(
                    concat!("msr ", stringify!($sysreg), ", {value}"),
                    options(nostack),
                    value = in(reg) value,
                );
            }
        }
    };
}

macro_rules! read_write_sysreg {
    ($sysreg:ident, $type:ty, safe $read_function_name:ident, safe $write_function_name:ident) => {
        read_sysreg!($sysreg, $type, safe $read_function_name);
        write_sysreg!($sysreg, $type, safe $write_function_name);
    };
    ($sysreg:ident, $type:ty, safe $read_function_name:ident, $write_function_name:ident) => {
        read_sysreg!($sysreg, $type, safe $read_function_name);
        write_sysreg!($sysreg, $type, $write_function_name);
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $read_function_name:ident, safe $write_function_name:ident) => {
        read_sysreg!($sysreg, $raw_type : $type, safe $read_function_name);
        write_sysreg!($sysreg, $raw_type : $type, safe $write_function_name);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $raw_type:ty : $type:ty, safe $read_function_name:ident, $write_function_name:ident
    ) => {
        read_sysreg!($sysreg, $raw_type : $type, safe $read_function_name);
        write_sysreg! {
            $(#[$attributes])*
            $sysreg, $raw_type : $type, $write_function_name
        }
    };
}

read_sysreg!(id_aa64mmfr1_el1, u64, safe read_id_aa64mmfr1_el1);
read_sysreg!(mpidr_el1, u64: MpidrEl1, safe read_mpidr_el1);
read_write_sysreg!(actlr_el1, u64, safe read_actlr_el1, safe write_actlr_el1);
read_write_sysreg!(actlr_el2, u64, safe read_actlr_el2, safe write_actlr_el2);
read_write_sysreg!(afsr0_el1, u64, safe read_afsr0_el1, safe write_afsr0_el1);
read_write_sysreg!(afsr0_el2, u64, safe read_afsr0_el2, safe write_afsr0_el2);
read_write_sysreg!(afsr1_el1, u64, safe read_afsr1_el1, safe write_afsr1_el1);
read_write_sysreg!(afsr1_el2, u64, safe read_afsr1_el2, safe write_afsr1_el2);
read_write_sysreg!(amair_el1, u64, safe read_amair_el1, safe write_amair_el1);
read_write_sysreg!(amair_el2, u64, safe read_amair_el2, safe write_amair_el2);
read_write_sysreg!(cntfrq_el0, u64, safe read_cntfrq_el0, safe write_cntfrq_el0);
read_write_sysreg!(cnthctl_el2, u64, safe read_cnthctl_el2, safe write_cnthctl_el2);
read_write_sysreg!(cntvoff_el2, u64, safe read_cntvoff_el2, safe write_cntvoff_el2);
read_write_sysreg!(contextidr_el1, u64, safe read_contextidr_el1, safe write_contextidr_el1);
read_write_sysreg!(contextidr_el2, u64, safe read_contextidr_el2, safe write_contextidr_el2);
read_write_sysreg!(cpacr_el1, u64, safe read_cpacr_el1, safe write_cpacr_el1);
read_write_sysreg!(cptr_el2, u64, safe read_cptr_el2, safe write_cptr_el2);
read_write_sysreg!(csselr_el1, u64, safe read_csselr_el1, safe write_csselr_el1);
read_write_sysreg!(elr_el1, usize, safe read_elr_el1, safe write_elr_el1);
read_write_sysreg!(elr_el2, usize, safe read_elr_el2, safe write_elr_el2);
read_write_sysreg!(esr_el1, u64: Esr, safe read_esr_el1, safe write_esr_el1);
read_write_sysreg!(esr_el2, u64: Esr, safe read_esr_el2, safe write_esr_el2);
read_write_sysreg!(far_el1, u64, safe read_far_el1, safe write_far_el1);
read_write_sysreg!(far_el2, u64, safe read_far_el2, safe write_far_el2);
read_write_sysreg!(hacr_el2, u64, safe read_hacr_el2, safe write_hacr_el2);
read_write_sysreg!(hcr_el2, u64: HcrEl2, safe read_hcr_el2, safe write_hcr_el2);
read_write_sysreg!(hpfar_el2, u64, safe read_hpfar_el2, safe write_hpfar_el2);
read_write_sysreg!(hstr_el2, u64, safe read_hstr_el2, safe write_hstr_el2);
read_write_sysreg!(icc_sre_el1, u64: IccSre, safe read_icc_sre_el1, safe write_icc_sre_el1);
read_write_sysreg!(icc_sre_el2, u64: IccSre, safe read_icc_sre_el2, safe write_icc_sre_el2);
write_sysreg! {
    /// # Safety
    ///
    /// The SRE bit of `icc_sre_el3` must not be changed from 1 to 0, as this can result in
    /// unpredictable behaviour.
    icc_sre_el3, u64: IccSre, write_icc_sre_el3
}
read_write_sysreg!(ich_hcr_el2, u64, safe read_ich_hcr_el2, safe write_ich_hcr_el2);
read_write_sysreg!(ich_vmcr_el2, u64, safe read_ich_vmcr_el2, safe write_ich_vmcr_el2);
read_sysreg!(isr_el1, u64, safe read_isr_el1);
read_write_sysreg!(mair_el1, u64, safe read_mair_el1, safe write_mair_el1);
read_write_sysreg!(mair_el2, u64, safe read_mair_el2, safe write_mair_el2);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a correct and safe configuration value for the EL3
    /// memory attribute indirection register.
    mair_el3, u64, write_mair_el3
}
read_write_sysreg!(mdccint_el1, u64, safe read_mdccint_el1, safe write_mdccint_el1);
read_write_sysreg!(mdcr_el2, u64, safe read_mdcr_el2, safe write_mdcr_el2);
read_write_sysreg!(mdscr_el1, u64, safe read_mdscr_el1, safe write_mdscr_el1);
read_sysreg!(midr_el1, u64, safe read_midr_el1);
read_write_sysreg!(par_el1, u64, safe read_par_el1, safe write_par_el1);
read_write_sysreg!(scr_el3, u64: ScrEl3, safe read_scr_el3, safe write_scr_el3);
read_write_sysreg!(sctlr_el1, u64: SctlrEl1, safe read_sctlr_el1, safe write_sctlr_el1);
read_write_sysreg!(sctlr_el2, u64, safe read_sctlr_el2, safe write_sctlr_el2);
read_write_sysreg! {
    /// # Safety
    ///
    /// Given its purpose, writing to the EL3 system control register can be very dangerous: it
    /// affects the behavior of the MMU, interrupt handling, security-relevant features like memory
    /// tagging, branch target identification, and pointer authentication, and more. Callers of
    /// `write_sctlr_el3` must ensure that the register value upholds TF-A security and reliability
    /// requirements.
    sctlr_el3, u64: SctlrEl3, safe read_sctlr_el3, write_sctlr_el3
}
read_write_sysreg!(sp_el1, u64, safe read_sp_el1, safe write_sp_el1);
read_write_sysreg!(sp_el2, u64, safe read_sp_el2, safe write_sp_el2);
read_write_sysreg!(spsr_el1, u64: Spsr, safe read_spsr_el1, safe write_spsr_el1);
read_write_sysreg!(spsr_el2, u64: Spsr, safe read_spsr_el2, safe write_spsr_el2);
read_write_sysreg!(tcr_el1, u64, safe read_tcr_el1, safe write_tcr_el1);
read_write_sysreg!(tcr_el2, u64, safe read_tcr_el2, safe write_tcr_el2);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a correct and safe configuration value for the EL3
    /// translation control register.
    tcr_el3, u64, write_tcr_el3
}
read_write_sysreg!(tpidr_el0, u64, safe read_tpidr_el0, safe write_tpidr_el0);
read_write_sysreg!(tpidr_el1, u64, safe read_tpidr_el1, safe write_tpidr_el1);
read_write_sysreg!(tpidr_el2, u64, safe read_tpidr_el2, safe write_tpidr_el2);
read_write_sysreg!(tpidrro_el0, u64, safe read_tpidrro_el0, safe write_tpidrro_el0);
read_write_sysreg!(ttbr0_el1, u64, safe read_ttbr0_el1, safe write_ttbr0_el1);
read_write_sysreg!(ttbr0_el2, u64, safe read_ttbr0_el2, safe write_ttbr0_el2);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a valid base address for the EL3 translation table:
    /// it must be page-aligned, and must point to a stage 1 translation table in the EL3
    /// translation regime.
    ttbr0_el3, usize, write_ttbr0_el3
}
read_write_sysreg!(ttbr1_el1, u64, safe read_ttbr1_el1, safe write_ttbr1_el1);
read_write_sysreg!(ttbr1_el2, u64, safe read_ttbr1_el2, safe write_ttbr1_el2);
read_write_sysreg!(vbar_el1, usize, safe read_vbar_el1, safe write_vbar_el1);
read_write_sysreg!(vbar_el2, usize, safe read_vbar_el2, safe write_vbar_el2);
read_write_sysreg!(vmpidr_el2, u64, safe read_vmpidr_el2, safe write_vmpidr_el2);
read_write_sysreg!(vpidr_el2, u64, safe read_vpidr_el2, safe write_vpidr_el2);
read_write_sysreg!(vtcr_el2, u64, safe read_vtcr_el2, safe write_vtcr_el2);
read_write_sysreg!(vttbr_el2, u64, safe read_vttbr_el2, safe write_vttbr_el2);

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ScrEl3: u64 {
        /// RES1 bits in the `scr_el3` register.
        const RES1 = (1 << 4) | (1 << 5);
        const NS = 1 << 0;
        const IRQ = 1 << 1;
        const FIQ = 1 << 2;
        const EA = 1 << 3;
        const SMD = 1 << 7;
        const HCE = 1 << 8;
        const SIF = 1 << 9;
        const RW = 1 << 10;
        const ST = 1 << 11;
        const TWI = 1 << 12;
        const TWE = 1 << 13;
        const TLOR = 1 << 14;
        const TERR = 1 << 15;
        const APK = 1 << 16;
        const API = 1 << 17;
        const EEL2 = 1 << 18;
        const EASE = 1 << 19;
        const NMEA = 1 << 20;
        const FIEN = 1 << 21;
        const TID3 = 1 << 22;
        const TID5 = 1 << 23;
        const ENSCXT = 1 << 25;
        const ATA = 1 << 26;
        const FGTEN = 1 << 27;
        const ECVEN = 1 << 28;
        const TWEDEN = 1 << 29;
        const TME = 1 << 34;
        const AMVOFFEN = 1 << 35;
        const ENAS0 = 1 << 36;
        const ADEN = 1 << 37;
        const HXEN = 1 << 38;
        const GCSEN = 1 << 39;
        const TRNDR = 1 << 40;
        const ENTP2 = 1 << 41;
        const RCWMASKEN = 1 << 42;
        const TCR2EN = 1 << 43;
        const SCTLR2EN = 1 << 44;
        const PIEN = 1 << 45;
        const AIEN = 1 << 46;
        const D128EN = 1 << 47;
        const GPF = 1 << 48;
        const MECEN = 1 << 49;
        const ENFPM = 1 << 50;
        const TMEA = 1 << 51;
        const TWERR = 1 << 52;
        const PFAREN = 1 << 53;
        const SRMASKEN = 1 << 54;
        const ENIDCP128 = 1 << 55;
        const DSE = 1 << 57;
        const ENDSE = 1 << 58;
        const FGTEN2 = 1 << 59;
        const HDBSSEN = 1 << 60;
        const HACDBSEN = 1 << 61;
        const NSE = 1 << 62;
    }

    /// Type for the `icc_sre_el2` and `icc_sre_el3` registers.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct IccSre: u64 {
        const SRE = 1 << 0;
        const DFB = 1 << 1;
        const DIB = 1 << 2;
        const EN = 1 << 3;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SctlrEl1: u64 {
        /// RES1 bits in the `sctlr_el1` register.
        const RES1 = (1 << 29) | (1 << 28) | (1 << 23) | (1 << 22) | (1 << 20) | (1 << 11);
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SctlrEl3: u64 {
        /// MMU enable for EL3 stage 1 address translation.
        const M = 1 << 0;
        /// Alignment check enable.
        const A = 1 << 1;
        /// Cacheability control, for data accesses at EL3.
        const C = 1 << 2;
        /// SP alignment check enable.
        const SA = 1 << 3;
        /// Cacheability control, for instruction accesses at EL3.
        const I = 1 << 12;
        /// Write permission implies XN (Execute-never). For the EL3 translation regime, this bit
        /// can force all memory regions that are writable to be treated as XN.
        const WXN = 1 << 19;
        /// RES1 bits in the `sctlr_el3` register.
        const RES1 = (1 << 23) | (1 << 18);
        const ENIB = 1 << 30;
        const ENIA = 1 << 31;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct HcrEl2: u64 {
        const TGE = 1 << 27;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
    pub struct CptrEl3: u64 {
        /// Trap EL2 accesses to CPTR_EL2/HCPTR, and EL2/EL1 accesses to CPACR_EL1/CPACR.
        const TCPAC = 1 << 31;
        /// When FEAT_AMUv1 implemented and, trap accesses from EL2/EL1/EL0 to AMU registers.
        const TAM = 1 << 30;
        /// Ttrap trace system register accesses.
        const TTA = 1 << 20;
        /// When FEAT_SME is implemented, do not trap SME instructions and system registers
        /// accesses.
        const ESM = 1 << 12;
        /// Trap Advanced SIMD instructions execution.
        const TFP = 1 << 10;
        /// Do not trap execution of SVE instructions.
        const EZ = 1 << 8;
    }
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExceptionLevel {
    El0 = 0,
    El1 = 1,
    El2 = 2,
    El3 = 3,
}

/// Values for SPSEL.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StackPointer {
    El0 = 0,
    ElX = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Spsr(u64);

bitflagslike!(Spsr: u64);

impl Spsr {
    const EL_MASK: u64 = 0x3;
    const EL_SHIFT: usize = 2;
    const SP_MASK: u64 = 0x1;

    /// AArch64 execution state, EL0.
    pub const M_AARCH64_EL0: Self = Self(0b00000);
    /// AArch64 execution state, EL1 with SP_EL0.
    pub const M_AARCH64_EL1T: Self = Self(0b00100);
    /// AArch64 execution state, EL1 with SP_EL1.
    pub const M_AARCH64_EL1H: Self = Self(0b00101);
    /// AArch64 execution state, EL2 with SP_EL0.
    pub const M_AARCH64_EL2T: Self = Self(0b01000);
    /// AArch64 execution state, EL2 with SP_EL2.
    pub const M_AARCH64_EL2H: Self = Self(0b01001);
    /// AArch64 execution state, EL3 with SP_EL0.
    pub const M_AARCH64_EL3T: Self = Self(0b01100);
    /// AArch64 execution state, EL3 with SP_EL3.
    pub const M_AARCH64_EL3H: Self = Self(0b01101);

    /// Exception was taken with PSTATE.SP set to SP_EL0.
    pub const SP_EL0: Self = Self(0);
    /// Exception was taken with PSTATE.SP set to SP_ELx.
    pub const SP_ELX: Self = Self(1);

    /// Exception was taken from AArch32 state.
    pub const M_EXECUTION_STATE: Self = Self(1 << 4);

    /// FIQ interrupt mask.
    pub const F: Self = Self(1 << 6);
    /// IRQ interrupt mask.
    pub const I: Self = Self(1 << 7);
    /// SError exception mask.
    pub const A: Self = Self(1 << 8);
    /// Debug exception mask.
    pub const D: Self = Self(1 << 9);

    /// Illegal Execution state.
    pub const IL: Self = Self(1 << 20);
    /// Software Step.
    pub const SS: Self = Self(1 << 21);

    pub const DIT: Self = Self(1 << 24);

    pub const V: Self = Self(1 << 28);
    pub const C: Self = Self(1 << 29);
    pub const Z: Self = Self(1 << 30);
    pub const N: Self = Self(1 << 31);
    pub const NZCV: Self = Self(Spsr::V.0 | Spsr::C.0 | Spsr::Z.0 | Spsr::N.0);

    pub const fn exception_level(self) -> ExceptionLevel {
        match (self.0 >> Self::EL_SHIFT) & Self::EL_MASK {
            0 => ExceptionLevel::El0,
            1 => ExceptionLevel::El1,
            2 => ExceptionLevel::El2,
            3 => ExceptionLevel::El3,
            _ => unreachable!(),
        }
    }

    pub const fn stack_pointer(self) -> StackPointer {
        match self.0 & Self::SP_MASK {
            0 => StackPointer::El0,
            1 => StackPointer::ElX,
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct Esr(u64);

impl Debug for Esr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Esr({:#x})", self.0)
    }
}

bitflagslike!(Esr: u64);

impl Esr {
    pub const ISS_SYSREG_OPCODE_MASK: Self = Self(0x003f_fc1e);
    pub const IL: Self = Self(1 << 25);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MpidrEl1(u64);

bitflagslike!(MpidrEl1: u64);

impl MpidrEl1 {
    pub const AFF0_MASK: u64 = 0x0000_00ff;
    pub const AFF1_MASK: u64 = 0x0000_ff00;
    pub const AFFINITY_BITS: usize = 8;
    pub const AFF0_SHIFT: u8 = 0;
    pub const AFF1_SHIFT: u8 = 8;
    pub const AFF2_SHIFT: u8 = 16;
    pub const AFF3_SHIFT: u8 = 32;
    pub const MT: Self = Self(1 << 24);
    pub const U: Self = Self(1 << 30);

    /// Converts a PSCI MPIDR value into the equivalent `MpidrEL1` value.
    ///
    /// This reads the MT and U bits from the current CPU's MPIDR_EL1 value and combines them with
    /// the affinity values from the given `psci_mpidr`.
    ///
    /// This assumes that the MPIDR_EL1 values of all CPUs in a system have the same values for the
    /// MT and U bits.
    pub fn from_psci_mpidr(psci_mpidr: u64) -> Self {
        let mpidr_el1 = read_mpidr_el1();
        Self(psci_mpidr) | (mpidr_el1 & (Self::MT | Self::U))
    }

    pub fn aff0(self) -> u8 {
        (self.0 >> Self::AFF0_SHIFT) as u8
    }

    pub fn aff1(self) -> u8 {
        (self.0 >> Self::AFF1_SHIFT) as u8
    }

    pub fn aff2(self) -> u8 {
        (self.0 >> Self::AFF2_SHIFT) as u8
    }

    pub fn aff3(self) -> u8 {
        (self.0 >> Self::AFF3_SHIFT) as u8
    }

    pub fn mt(self) -> bool {
        self & Self::MT != Self::empty()
    }
}

pub fn is_feat_vhe_present() -> bool {
    const VHE: u64 = 1 << 8;

    read_id_aa64mmfr1_el1() & VHE != 0
}
