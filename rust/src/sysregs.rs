// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
#[macro_use]
pub mod fake;

#[cfg(test)]
pub use fake::write_sp_el3;

#[cfg(not(test))]
use core::arch::asm;

/// Generates a public function named `$function_name` to read the system register `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to read.
#[cfg(not(test))]
macro_rules! read_sysreg {
    ($sysreg:ident, safe $function_name:ident) => {
        pub fn $function_name() -> u64 {
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
    ($sysreg:ident, $function_name:ident) => {
        pub unsafe fn $function_name() -> u64 {
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
}

/// Generates a public function named `$function_name` to write to the system register
/// `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to write any value
/// to.
#[cfg(not(test))]
macro_rules! write_sysreg {
    ($sysreg:ident, safe $function_name:ident) => {
        pub fn $function_name(value: u64) {
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
    ($sysreg:ident, $function_name:ident) => {
        pub unsafe fn $function_name(value: u64) {
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
    ($sysreg:ident, safe $read_function_name:ident, safe $write_function_name:ident) => {
        read_sysreg!($sysreg, safe $read_function_name);
        write_sysreg!($sysreg, safe $write_function_name);
    };
    ($sysreg:ident, safe $read_function_name:ident, $write_function_name:ident) => {
        read_sysreg!($sysreg, safe $read_function_name);
        write_sysreg!($sysreg, $write_function_name);
    };
}

read_write_sysreg!(actlr_el2, safe read_actlr_el2, safe write_actlr_el2);
read_write_sysreg!(afsr0_el2, safe read_afsr0_el2, safe write_afsr0_el2);
read_write_sysreg!(afsr1_el2, safe read_afsr1_el2, safe write_afsr1_el2);
read_write_sysreg!(amair_el2, safe read_amair_el2, safe write_amair_el2);
read_write_sysreg!(cnthctl_el2, safe read_cnthctl_el2, safe write_cnthctl_el2);
read_write_sysreg!(cntvoff_el2, safe read_cntvoff_el2, safe write_cntvoff_el2);
read_write_sysreg!(cptr_el2, safe read_cptr_el2, safe write_cptr_el2);
read_write_sysreg!(elr_el1, safe read_elr_el1, safe write_elr_el1);
read_write_sysreg!(elr_el2, safe read_elr_el2, safe write_elr_el2);
read_write_sysreg!(esr_el1, safe read_esr_el1, safe write_esr_el1);
read_write_sysreg!(esr_el2, safe read_esr_el2, safe write_esr_el2);
read_write_sysreg!(far_el2, safe read_far_el2, safe write_far_el2);
read_write_sysreg!(hacr_el2, safe read_hacr_el2, safe write_hacr_el2);
read_write_sysreg!(hcr_el2, safe read_hcr_el2, safe write_hcr_el2);
read_write_sysreg!(hpfar_el2, safe read_hpfar_el2, safe write_hpfar_el2);
read_write_sysreg!(hstr_el2, safe read_hstr_el2, safe write_hstr_el2);
read_write_sysreg!(icc_sre_el2, safe read_icc_sre_el2, safe write_icc_sre_el2);
read_write_sysreg!(ich_hcr_el2, safe read_ich_hcr_el2, safe write_ich_hcr_el2);
read_write_sysreg!(ich_vmcr_el2, safe read_ich_vmcr_el2, safe write_ich_vmcr_el2);
read_write_sysreg!(mair_el2, safe read_mair_el2, safe write_mair_el2);
read_write_sysreg!(mdcr_el2, safe read_mdcr_el2, safe write_mdcr_el2);
read_write_sysreg!(scr_el3, safe read_scr_el3, safe write_scr_el3);
read_write_sysreg!(sctlr_el1, safe read_sctlr_el1, safe write_sctlr_el1);
read_write_sysreg!(sctlr_el2, safe read_sctlr_el2, safe write_sctlr_el2);
read_write_sysreg!(sp_el2, safe read_sp_el2, safe write_sp_el2);
read_write_sysreg!(spsr_el1, safe read_spsr_el1, safe write_spsr_el1);
read_write_sysreg!(spsr_el2, safe read_spsr_el2, safe write_spsr_el2);
read_write_sysreg!(tcr_el2, safe read_tcr_el2, safe write_tcr_el2);
read_write_sysreg!(tpidr_el2, safe read_tpidr_el2, safe write_tpidr_el2);
read_write_sysreg!(ttbr0_el2, safe read_ttbr0_el2, safe write_ttbr0_el2);
read_write_sysreg!(vbar_el1, safe read_vbar_el1, safe write_vbar_el1);
read_write_sysreg!(vbar_el2, safe read_vbar_el2, safe write_vbar_el2);
read_write_sysreg!(vmpidr_el2, safe read_vmpidr_el2, safe write_vmpidr_el2);
read_write_sysreg!(vpidr_el2, safe read_vpidr_el2, safe write_vpidr_el2);
read_write_sysreg!(vtcr_el2, safe read_vtcr_el2, safe write_vtcr_el2);
read_write_sysreg!(vttbr_el2, safe read_vttbr_el2, safe write_vttbr_el2);
// The SRE bit of `icc_sre_el3` must not be changed from 1 to 0, as this can result in unpredictable
// behaviour.
write_sysreg!(icc_sre_el3, write_icc_sre_el3);

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
