// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
#[macro_use]
pub mod fake;

#[cfg(not(test))]
use core::arch::asm;

/// Generates a safe public function named `$function_name` to read the system register `$sysreg`.
///
/// This should only be used for system registers which are indeed safe to read.
#[cfg(not(test))]
macro_rules! read_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name() -> u64 {
            let value;
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
            // to read the given `$sysreg.`
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

/// Generates a safe public function named `$function_name` to write to the system register
/// `$sysreg`.
///
/// This should only be used for system registers which are indeed safe to write.
#[cfg(not(test))]
macro_rules! write_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name(value: u64) {
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
            // to write `value` to the given `$sysreg.`
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

read_sysreg!(actlr_el2, read_actlr_el2);
read_sysreg!(afsr0_el2, read_afsr0_el2);
read_sysreg!(afsr1_el2, read_afsr1_el2);
read_sysreg!(amair_el2, read_amair_el2);
read_sysreg!(cnthctl_el2, read_cnthctl_el2);
read_sysreg!(cntvoff_el2, read_cntvoff_el2);
read_sysreg!(cptr_el2, read_cptr_el2);
read_sysreg!(elr_el1, read_elr_el1);
read_sysreg!(elr_el2, read_elr_el2);
read_sysreg!(esr_el1, read_esr_el1);
read_sysreg!(esr_el2, read_esr_el2);
read_sysreg!(far_el2, read_far_el2);
read_sysreg!(hacr_el2, read_hacr_el2);
read_sysreg!(hcr_el2, read_hcr_el2);
read_sysreg!(hpfar_el2, read_hpfar_el2);
read_sysreg!(hstr_el2, read_hstr_el2);
read_sysreg!(icc_sre_el2, read_icc_sre_el2);
read_sysreg!(ich_hcr_el2, read_ich_hcr_el2);
read_sysreg!(ich_vmcr_el2, read_ich_vmcr_el2);
read_sysreg!(mair_el2, read_mair_el2);
read_sysreg!(mdcr_el2, read_mdcr_el2);
read_sysreg!(sctlr_el1, read_sctlr_el1);
read_sysreg!(sctlr_el2, read_sctlr_el2);
read_sysreg!(sp_el2, read_sp_el2);
read_sysreg!(spsr_el1, read_spsr_el1);
read_sysreg!(spsr_el2, read_spsr_el2);
read_sysreg!(tcr_el2, read_tcr_el2);
read_sysreg!(tpidr_el2, read_tpidr_el2);
read_sysreg!(ttbr0_el2, read_ttbr0_el2);
read_sysreg!(vbar_el1, read_vbar_el1);
read_sysreg!(vbar_el2, read_vbar_el2);
read_sysreg!(vmpidr_el2, read_vmpidr_el2);
read_sysreg!(vpidr_el2, read_vpidr_el2);
read_sysreg!(vtcr_el2, read_vtcr_el2);
read_sysreg!(vttbr_el2, read_vttbr_el2);

write_sysreg!(actlr_el2, write_actlr_el2);
write_sysreg!(afsr0_el2, write_afsr0_el2);
write_sysreg!(afsr1_el2, write_afsr1_el2);
write_sysreg!(amair_el2, write_amair_el2);
write_sysreg!(cnthctl_el2, write_cnthctl_el2);
write_sysreg!(cntvoff_el2, write_cntvoff_el2);
write_sysreg!(cptr_el2, write_cptr_el2);
write_sysreg!(elr_el1, write_elr_el1);
write_sysreg!(elr_el2, write_elr_el2);
write_sysreg!(esr_el1, write_esr_el1);
write_sysreg!(esr_el2, write_esr_el2);
write_sysreg!(far_el2, write_far_el2);
write_sysreg!(hacr_el2, write_hacr_el2);
write_sysreg!(hcr_el2, write_hcr_el2);
write_sysreg!(hpfar_el2, write_hpfar_el2);
write_sysreg!(hstr_el2, write_hstr_el2);
write_sysreg!(icc_sre_el2, write_icc_sre_el2);
write_sysreg!(ich_hcr_el2, write_ich_hcr_el2);
write_sysreg!(ich_vmcr_el2, write_ich_vmcr_el2);
write_sysreg!(mair_el2, write_mair_el2);
write_sysreg!(mdcr_el2, write_mdcr_el2);
write_sysreg!(sctlr_el1, write_sctlr_el1);
write_sysreg!(sctlr_el2, write_sctlr_el2);
write_sysreg!(sp_el2, write_sp_el2);
write_sysreg!(spsr_el1, write_spsr_el1);
write_sysreg!(spsr_el2, write_spsr_el2);
write_sysreg!(tcr_el2, write_tcr_el2);
write_sysreg!(tpidr_el2, write_tpidr_el2);
write_sysreg!(ttbr0_el2, write_ttbr0_el2);
write_sysreg!(vbar_el1, write_vbar_el1);
write_sysreg!(vbar_el2, write_vbar_el2);
write_sysreg!(vmpidr_el2, write_vmpidr_el2);
write_sysreg!(vpidr_el2, write_vpidr_el2);
write_sysreg!(vtcr_el2, write_vtcr_el2);
write_sysreg!(vttbr_el2, write_vttbr_el2);
