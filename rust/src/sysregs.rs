// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
#[macro_use]
pub mod fake;

use core::arch::asm;

/// Generates a safe public function named `$function_name` to read the system register `$sysreg`.
///
/// This should only be used for system registers which are indeed safe to read.
#[cfg(not(test))]
macro_rules! read_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name() -> u64 {
            let value;
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

read_sysreg!(vbar_el1, read_vbar_el1);
read_sysreg!(vbar_el2, read_vbar_el2);
read_sysreg!(hcr_el2, read_hcr_el2);

write_sysreg!(esr_el1, write_esr_el1);
write_sysreg!(esr_el2, write_esr_el2);
write_sysreg!(elr_el1, write_elr_el1);
write_sysreg!(elr_el2, write_elr_el2);
write_sysreg!(spsr_el1, write_spsr_el1);
write_sysreg!(spsr_el2, write_spsr_el2);
write_sysreg!(sctlr_el1, write_sctlr_el1);
