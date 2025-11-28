// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_gic::IntId;
use arm_sysregs::write_sysreg;
use bitflags::bitflags;

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
}

/// An implementation for the ARM Generic **Non-secure** Physical Timer.
pub struct NonSecureTimer;

impl Timer for NonSecureTimer {
    const INTERRUPT_ID: IntId = IntId::ppi(14);

    fn write_timer_value(ticks: u32) {
        write_cntp_tval_el0(ticks as u64);
    }

    fn write_timer_control(value: TimerControl) {
        write_cntp_ctl_el0(value);
    }
}

/// An implementation for the ARM Generic **Secure** EL1 Physical Timer.
pub struct SEL1Timer;

impl Timer for SEL1Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(13);

    fn write_timer_value(ticks: u32) {
        write_cntps_tval_el1(ticks as u64);
    }

    fn write_timer_control(value: TimerControl) {
        write_cntps_ctl_el1(value);
    }
}

/// An implementation for the ARM Generic **Secure** EL2 Physical Timer.
pub struct SEL2Timer;

impl Timer for SEL2Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(4);

    fn write_timer_value(ticks: u32) {
        write_s3_4_c14_c5_0(ticks as u64);
    }

    fn write_timer_control(value: TimerControl) {
        write_s3_4_c14_c5_1(value);
    }
}

write_sysreg!(cntp_tval_el0, u64, safe);
write_sysreg!(cntp_ctl_el0, u64: TimerControl, safe);
write_sysreg!(cntps_tval_el1, u64, safe);
write_sysreg!(cntps_ctl_el1, u64: TimerControl, safe);
write_sysreg!(s3_4_c14_c5_0, u64, safe);
write_sysreg!(s3_4_c14_c5_1, u64: TimerControl, safe);
