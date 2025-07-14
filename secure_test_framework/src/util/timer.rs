// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_gic::IntId;
use bitflags::bitflags;
use core::arch::asm;

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

/// A macro to generate an MRS instruction to read from a system register.
macro_rules! read_sysreg {
    ($reg:ident) => {{
        let value: u64;
        asm!(
            concat!("mrs {}, ", stringify!($reg)),
            out(reg) value,
            options(nostack, nomem, preserves_flags)
        );
        value
    }};
}

/// Reads the frequency of the system counter from the `CNTFRQ_EL0` register.
/// It is assumed this has been initialized by EL3 firmware.
pub fn read_frequency() -> u64 {
    // SAFETY: This only reads a readable timer system register.
    unsafe { read_sysreg!(cntfrq_el0) }
}

/// A hint to the processor that we are in a spin-wait loop.
///
/// This can improve performance and save power on some systems.
#[inline(always)]
pub fn cpu_yield() {
    // SAFETY: The 'yield' instruction is a CPU hint that is guaranteed
    // to have no memory side effects. It does not use the stack and
    // preserves all general-purpose and flag registers.
    unsafe {
        asm!("yield", options(nomem, nostack, preserves_flags));
    }
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

    fn delay_us(us: u64) {
        let freq = read_frequency();
        if freq == 0 {
            panic!("CNTFREQ_EL0 not configured/inaccessible");
        }

        // Calculate the number of timer ticks required for the delay.
        // Use u128 for the multiplication to prevent overflow.
        let ticks_to_wait = (freq as u128 * us as u128) / 1_000_000;
        let start_time = Self::read_counter();
        let end_time = start_time.saturating_add(ticks_to_wait as u64);

        // Loop until the system counter reaches the target time.
        while Self::read_counter() < end_time {
            // Hint the processor that we are in a spin-wait loop.
            // This can save power on some systems.
            cpu_yield();
        }
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
