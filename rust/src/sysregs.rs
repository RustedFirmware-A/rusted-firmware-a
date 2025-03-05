// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
#[macro_use]
pub mod fake;

#[cfg(test)]
pub use fake::write_sp_el3;

use bitflags::bitflags;
#[cfg(not(test))]
use core::arch::asm;
use core::ops::BitOr;

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

read_write_sysreg!(actlr_el1, u64, safe read_actlr_el1, safe write_actlr_el1);
read_write_sysreg!(actlr_el2, u64, safe read_actlr_el2, safe write_actlr_el2);
read_write_sysreg!(afsr0_el1, u64, safe read_afsr0_el1, safe write_afsr0_el1);
read_write_sysreg!(afsr0_el2, u64, safe read_afsr0_el2, safe write_afsr0_el2);
read_write_sysreg!(afsr1_el1, u64, safe read_afsr1_el1, safe write_afsr1_el1);
read_write_sysreg!(afsr1_el2, u64, safe read_afsr1_el2, safe write_afsr1_el2);
read_write_sysreg!(amair_el1, u64, safe read_amair_el1, safe write_amair_el1);
read_write_sysreg!(amair_el2, u64, safe read_amair_el2, safe write_amair_el2);
read_write_sysreg!(cnthctl_el2, u64, safe read_cnthctl_el2, safe write_cnthctl_el2);
read_write_sysreg!(cntvoff_el2, u64, safe read_cntvoff_el2, safe write_cntvoff_el2);
read_write_sysreg!(contextidr_el1, u64, safe read_contextidr_el1, safe write_contextidr_el1);
read_write_sysreg!(cpacr_el1, u64, safe read_cpacr_el1, safe write_cpacr_el1);
read_write_sysreg!(cptr_el2, u64, safe read_cptr_el2, safe write_cptr_el2);
read_write_sysreg!(csselr_el1, u64, safe read_csselr_el1, safe write_csselr_el1);
read_write_sysreg!(elr_el1, u64, safe read_elr_el1, safe write_elr_el1);
read_write_sysreg!(elr_el2, u64, safe read_elr_el2, safe write_elr_el2);
read_write_sysreg!(esr_el1, u64, safe read_esr_el1, safe write_esr_el1);
read_write_sysreg!(esr_el2, u64, safe read_esr_el2, safe write_esr_el2);
read_write_sysreg!(far_el1, u64, safe read_far_el1, safe write_far_el1);
read_write_sysreg!(far_el2, u64, safe read_far_el2, safe write_far_el2);
read_write_sysreg!(hacr_el2, u64, safe read_hacr_el2, safe write_hacr_el2);
read_write_sysreg!(hcr_el2, u64, safe read_hcr_el2, safe write_hcr_el2);
read_write_sysreg!(hpfar_el2, u64, safe read_hpfar_el2, safe write_hpfar_el2);
read_write_sysreg!(hstr_el2, u64, safe read_hstr_el2, safe write_hstr_el2);
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
read_write_sysreg!(spsr_el1, u64, safe read_spsr_el1, safe write_spsr_el1);
read_write_sysreg!(spsr_el2, u64, safe read_spsr_el2, safe write_spsr_el2);
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
read_write_sysreg!(vbar_el1, u64, safe read_vbar_el1, safe write_vbar_el1);
read_write_sysreg!(vbar_el2, u64, safe read_vbar_el2, safe write_vbar_el2);
read_write_sysreg!(vmpidr_el2, u64, safe read_vmpidr_el2, safe write_vmpidr_el2);
read_write_sysreg!(vpidr_el2, u64, safe read_vpidr_el2, safe write_vpidr_el2);
read_write_sysreg!(vtcr_el2, u64, safe read_vtcr_el2, safe write_vtcr_el2);
read_write_sysreg!(vttbr_el2, u64, safe read_vttbr_el2, safe write_vttbr_el2);

/// Writes `value` to `sp_el3`.
///
/// # Safety
///
/// The caller must ensure that `value` is consistent with how the rest of RF-A uses `sp_el3`.
#[cfg(not(test))]
pub unsafe fn write_sp_el3(value: usize) {
    // SAFETY: The caller guarantees that the value is a valid `sp_el3`.
    unsafe {
        asm!(
            "msr spsel, #1",
            "mov sp, {value}",
            "msr spsel, #0",
            value = in(reg) value,
        )
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ScrEl3: u64 {
        /// RES1 bits in the `scr_el3` register.
        const RES1 = 1 << 4 | 1 << 5;
        const NS = 1 << 0;
        const EA = 1 << 3;
        const HCE = 1 << 8;
        const SIF = 1 << 9;
        const RW = 1 << 10;
        const EEL2 = 1 << 18;
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
        const RES1 = 1 << 29 | 1 << 28 | 1 << 23 | 1 << 22 | 1 << 20 | 1 << 11;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SctlrEl3: u64 {
        /// MMU enable for EL3 stage 1 address translation.
        const M = 1 << 0;
        /// Cacheability control, for data accesses at EL3.
        const C = 1 << 2;
        /// Write permission implies XN (Execute-never). For the EL3 translation regime, this bit
        /// can force all memory regions that are writable to be treated as XN.
        const WXN = 1 << 19;
        /// RES1 bits in the `sctlr_el3` register.
        const RES1 = 1 << 23 | 1 << 18;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpsrEl3(u64);

impl SpsrEl3 {
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

    /// FIQ interrupt mask.
    pub const F: Self = Self(1 << 6);
    /// IRQ interrupt mask.
    pub const I: Self = Self(1 << 7);
    /// SError exception mask.
    pub const A: Self = Self(1 << 8);
    /// Debug exception mask.
    pub const D: Self = Self(1 << 9);

    pub const fn empty() -> Self {
        Self(0)
    }
}

impl BitOr for SpsrEl3 {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}
