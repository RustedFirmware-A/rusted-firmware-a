// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::gicv3;
use crate::util::current_el;
use core::arch::asm;

/// Sets the appropriate exception vector for the current exception level.
pub fn set_exception_vector() {
    match current_el() {
        // SAFETY: vector_table is a valid exception vector table, provided by aarch64-rt.
        1 => unsafe {
            asm!(
                "adr x9, vector_table_el1",
                "msr vbar_el1, x9",
                options(nomem, nostack),
                out("x9") _,
            );
        },
        // SAFETY: vector_table is a valid exception vector table, provided by aarch64-rt.
        2 => unsafe {
            asm!(
                "adr x9, vector_table_el2",
                "msr vbar_el2, x9",
                options(nomem, nostack),
                out("x9") _,
            );
        },
        el => panic!("Started at unexpected exception level {}", el),
    }
}

#[unsafe(no_mangle)]
extern "C" fn sync_exception_current(elr: u64, _spsr: u64) {
    panic!(
        "Unexpected sync_exception_current, esr={:#x}, far={:#x}, elr={:#x}",
        esr(),
        far(),
        elr
    );
}

#[unsafe(no_mangle)]
extern "C" fn irq_current(_elr: u64, _spsr: u64) {
    gicv3::handle_group1_interrupt();
}

#[unsafe(no_mangle)]
extern "C" fn fiq_current(_elr: u64, _spsr: u64) {
    panic!("Unexpected fiq_current");
}

#[unsafe(no_mangle)]
extern "C" fn serr_current(_elr: u64, _spsr: u64) {
    panic!("Unexpected serr_current");
}

#[unsafe(no_mangle)]
extern "C" fn sync_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected sync_lower");
}

#[unsafe(no_mangle)]
extern "C" fn irq_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected irq_lower");
}

#[unsafe(no_mangle)]
extern "C" fn fiq_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected fiq_lower");
}

#[unsafe(no_mangle)]
extern "C" fn serr_lower(_elr: u64, _spsr: u64) {
    panic!("Unexpected serr_lower");
}

fn esr() -> u64 {
    let mut esr: u64;
    if current_el() == 2 {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {esr}, esr_el2", esr = out(reg) esr);
        }
    } else {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {esr}, esr_el1", esr = out(reg) esr);
        }
    }
    esr
}

fn far() -> u64 {
    let mut far: u64;
    if current_el() == 2 {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {far}, far_el2", far = out(reg) far);
        }
    } else {
        // SAFETY: This only reads a system register.
        unsafe {
            asm!("mrs {far}, far_el1", far = out(reg) far);
        }
    }
    far
}
