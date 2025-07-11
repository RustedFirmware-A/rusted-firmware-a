// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::gicv3::set_interrupt_handler;
use arm_gic::IntId;
use bitflags::bitflags;
use core::{
    arch::asm,
    hint::spin_loop,
    sync::atomic::{AtomicBool, Ordering},
};
use log::debug;

bitflags! {
    /// Represents the control bits for the physical timers.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TimerControl: u64 {
        /// Bit 0: Enables the timer.
        const ENABLE = 1 << 0;
        /// Bit 1: Masks the timer interrupt.
        const IMASK = 1 << 1;
    }
}

/// A macro to generate an MSR instruction to write to a system register.
macro_rules! write_sysreg {
    ($reg:ident, $value:expr) => {
        asm!(
            concat!("msr ", stringify!($reg), ", {}"),
            in(reg) $value,
            options(nostack, nomem, preserves_flags)
        );
    };
}

/// Defines the common behavior for all physical timers.
pub trait Timer {
    /// The interrupt ID for this timer.
    const INTERRUPT_ID: IntId;

    /// Writes the interval value to the timer's TVAL register.
    fn write_timer_value(ticks: u32);

    /// Writes to the timer's control register.
    fn write_timer_control(value: TimerControl);

    /// Sets a one-shot timer to fire after a specific number of ticks.
    fn set(ticks: u32) {
        Self::write_timer_value(ticks);
        Self::write_timer_control(TimerControl::ENABLE);
    }

    /// Stops the timer.
    fn stop() {
        Self::write_timer_control(TimerControl::empty());
    }

    /// Reads the shared 64-bit physical system counter.
    fn read_counter() -> u64 {
        let value: u64;
        // SAFETY: This only reads a readable timer system register.
        unsafe {
            asm!("mrs {}, cntpct_el0", out(reg) value, options(nostack, nomem, preserves_flags));
        }
        value
    }
}

/// An implementation for the ARM Generic **Non-secure** Physical Timer.
pub struct NonSecureTimer;

impl Timer for NonSecureTimer {
    const INTERRUPT_ID: IntId = IntId::ppi(14);

    fn write_timer_value(ticks: u32) {
        // SAFETY: This only writes a writable timer system register.
        unsafe {
            write_sysreg!(cntp_tval_el0, ticks as u64);
        }
    }

    fn write_timer_control(value: TimerControl) {
        // SAFETY: This only writes a writable timer control system register.
        unsafe {
            write_sysreg!(cntp_ctl_el0, value.bits());
        }
    }
}

/// An implementation for the ARM Generic **Secure** EL1 Physical Timer.
pub struct SEL1Timer;

impl Timer for SEL1Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(13);

    fn write_timer_value(ticks: u32) {
        // SAFETY: This only writes a writable timer system register.
        unsafe {
            write_sysreg!(cntps_tval_el1, ticks as u64);
        }
    }

    fn write_timer_control(value: TimerControl) {
        // SAFETY: This only writes a writable timer control system register.
        unsafe {
            write_sysreg!(cntps_ctl_el1, value.bits());
        }
    }
}

/// An implementation for the ARM Generic **Secure** EL2 Physical Timer.
pub struct SEL2Timer;

impl Timer for SEL2Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(4);

    fn write_timer_value(ticks: u32) {
        // SAFETY: This only writes a writable timer system register.
        unsafe {
            write_sysreg!(S3_4_c14_c5_0, ticks as u64);
        }
    }

    fn write_timer_control(value: TimerControl) {
        // SAFETY: This only writes a writable timer control system register.
        unsafe {
            write_sysreg!(S3_4_c14_c5_1, value.bits());
        }
    }
}

/// TODO: move this test to a test module shared between NS and S worlds.
///
/// A generic helper that runs the timer test logic for any type `TIMER`
/// that implements the `Timer` trait.
pub fn test_timer_helper<TIMER: Timer>() -> Result<(), ()> {
    // This flag can be safely accessed by both the main loop and an interrupt handler.
    // To be successfully registered as an interrupt handler, a closure cannot take arguments,
    // and therefore cannot capture local environment.
    // For that reason TIMER_HANDLED has to be made static.
    static TIMER_HANDLED: AtomicBool = AtomicBool::new(false);

    // Init again to avoid nasty bugs when this helper is used multiple times.
    TIMER_HANDLED.store(false, Ordering::Release);

    let timer_handler = || {
        debug!("Stopping timer");
        TIMER::stop();
        TIMER_HANDLED.store(true, Ordering::Release);
    };

    // Register the custom handler.
    // The closure can be passed as a function pointer.
    set_interrupt_handler(TIMER::INTERRUPT_ID, Some(timer_handler));

    // Configure the specific timer `TIMER`.
    TIMER::set(1_000_000);

    // Wait until the timer is handled.
    while !TIMER_HANDLED.load(Ordering::Acquire) {
        spin_loop();
    }

    // Clear the handler for timer interrupt.
    set_interrupt_handler(TIMER::INTERRUPT_ID, None);

    Ok(())
}
