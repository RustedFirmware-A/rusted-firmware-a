// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{gicv3, util::current_el};
use aarch64_rt::{ExceptionHandlers, RegisterStateRef, exception_handlers};
use core::arch::asm;

/// Without this flag the CPU assumes the IRQ is meant for EL1.
pub fn enable_irq_trapping_to_el2() {
    let hcr_el2: u64;

    // SAFETY: only RW system register hcr_el2 is accessed.
    unsafe {
        // Read the current value
        asm!("mrs {}, hcr_el2", out(reg) hcr_el2, options(nomem, nostack));

        // Set the IMO bit (bit 4)
        let new_hcr_el2 = hcr_el2 | (1 << 4);

        // Write the new value back
        asm!(
            "msr hcr_el2, {}",
            "dsb sy",
            "isb",
            in(reg) new_hcr_el2,
            options(nomem, nostack)
        );
    }
}

exception_handlers!(Exceptions);
struct Exceptions;

impl ExceptionHandlers for Exceptions {
    extern "C" fn sync_current(register_state: RegisterStateRef) {
        panic!(
            "Unexpected sync_current, esr={:#x}, far={:#x}, elr={:#x}",
            esr(),
            far(),
            register_state.elr
        );
    }

    extern "C" fn irq_current(_register_state: RegisterStateRef) {
        gicv3::handle_group1_interrupt();
    }

    extern "C" fn fiq_current(_register_state: RegisterStateRef) {
        panic!("Unexpected fiq_current");
    }

    extern "C" fn serror_current(_register_state: RegisterStateRef) {
        panic!("Unexpected serror_current");
    }

    extern "C" fn sync_lower(_register_state: RegisterStateRef) {
        panic!("Unexpected sync_lower");
    }

    extern "C" fn irq_lower(_register_state: RegisterStateRef) {
        panic!("Unexpected irq_lower");
    }

    extern "C" fn fiq_lower(_register_state: RegisterStateRef) {
        panic!("Unexpected fiq_lower");
    }

    extern "C" fn serror_lower(_register_state: RegisterStateRef) {
        panic!("Unexpected serror_lower");
    }
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
