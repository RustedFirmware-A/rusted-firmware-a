// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{exceptions::enable_irq_trapping_to_el2, util::current_el};
use arm_gic::{
    IntId,
    gicv3::{GicV3, InterruptGroup},
};
use core::arch::asm;
use log::debug;
use percore::{ExceptionLock, exception_free};
use spin::mutex::SpinMutex;

// Only private interrupts are supported.
const MAX_INTERRUPT_ID: usize = 31;

type InterruptHandler = fn();

/// To easily access the handler for any private interrupt.
/// Wrapped in ExceptionLock to avoid deadlocks when interrupt fires
/// when the lock is held.
static INTERRUPT_HANDLERS: ExceptionLock<
    SpinMutex<[Option<InterruptHandler>; MAX_INTERRUPT_ID + 1]>,
> = ExceptionLock::new(SpinMutex::new([None; MAX_INTERRUPT_ID + 1]));

/// Use linear interrupt id for indexing handler array.
fn get_interrupt_handler_idx(int_id: IntId) -> usize {
    u32::from(int_id) as usize
}

/// Configures the interrupt handler for interrupt with id `int_id`.
///
/// If `callback` is Some(fn), then `fn` will be called between ACK and EOI.
///
/// If callback is None, then any previously registered interrupt handler will be erased.
/// This should be used for clean-up between tests.
pub fn set_interrupt_handler(int_id: IntId, callback: Option<InterruptHandler>) {
    if !int_id.is_private() {
        panic!("Only private interrupts are supported.");
    }

    let idx = get_interrupt_handler_idx(int_id);
    exception_free(|token| {
        INTERRUPT_HANDLERS.borrow(token).lock()[idx] = callback;
    });
}

/// Acknowledges the interrupt, calls corresponding handler function and sets EOI.
pub fn handle_group1_interrupt() {
    let int_id = GicV3::get_and_acknowledge_interrupt(InterruptGroup::Group1).unwrap();
    debug!("Group 1 Secure interrupt {:?} acknowledged", int_id);

    if !int_id.is_private() {
        panic!("Only private interrupts are supported.");
    }

    let idx = get_interrupt_handler_idx(int_id);
    let handler = exception_free(|token| INTERRUPT_HANDLERS.borrow(token).lock()[idx]);

    if let Some(handler_fn) = handler {
        handler_fn();
    } else {
        panic!("No handler registered for interrupt {:?}", int_id);
    }

    GicV3::end_interrupt(int_id, InterruptGroup::Group1);
    debug!("Group 1 Secure interrupt {:?} EOI", int_id);
}

fn write_icc_sre(value: u64) {
    if current_el() == 2 {
        // SAFETY: This only writes a system register.
        unsafe {
            asm!("msr icc_sre_el2, {}", in(reg) value, options(nostack, nomem));
        }
    } else {
        // SAFETY: This only writes a system register.
        unsafe {
            asm!("msr icc_sre_el1, {}", in(reg) value, options(nostack, nomem));
        }
    }
}

/// Enables IRQ handling for the current EL.
pub fn init() {
    if current_el() == 2 {
        enable_irq_trapping_to_el2();
    }

    // Enable system register access (bit 0 = SRE).
    write_icc_sre(0x01);

    GicV3::enable_group1(true);
    arm_gic::irq_enable();
}
