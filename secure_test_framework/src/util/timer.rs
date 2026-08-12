// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_generic_timer::{
    Timer as GenericTimer, TimerInterface,
    sysreg::{PhysicalSecureTimer, PhysicalTimer, SecureEl2PhysicalTimer},
};
use arm_gic::IntId;

/// Defines an interface for the underlying GenericTimer and Interrupt ID for Physical Timers used
/// in tests.
pub trait Timer {
    /// The interrupt ID for this timer.
    const INTERRUPT_ID: IntId;

    type TimerInterface: TimerInterface;

    /// Returns an instance of this timer.
    ///
    /// # Safety
    /// The caller must ensure that no other instance of this timer exists and that there is no
    /// concurrent access to the corresponding timer system registers.
    unsafe fn timer() -> GenericTimer<Self::TimerInterface>;
}

/// An implementation for the ARM Generic **Non-secure** Physical Timer.
pub struct NonSecureTimer;

impl Timer for NonSecureTimer {
    const INTERRUPT_ID: IntId = IntId::ppi(14);

    type TimerInterface = PhysicalTimer;

    unsafe fn timer() -> GenericTimer<Self::TimerInterface> {
        // SAFETY: the caller ensures that no other PhysicalTimer exists and that there is no
        // concurrent access to the CNTP_* system registers.
        let timer = unsafe { PhysicalTimer::new() };
        GenericTimer::new(timer)
    }
}

/// An implementation for the ARM Generic **Secure** EL1 Physical Timer.
pub struct SEL1Timer;

impl Timer for SEL1Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(13);

    type TimerInterface = PhysicalSecureTimer;

    unsafe fn timer() -> GenericTimer<Self::TimerInterface> {
        // SAFETY: the caller ensures that no other PhysicalSecureTimer exists and that there is no
        // concurrent access to the CNTPS_* system registers.
        let timer = unsafe { PhysicalSecureTimer::new() };
        GenericTimer::new(timer)
    }
}

/// An implementation for the ARM Generic **Secure** EL2 Physical Timer.
pub struct SEL2Timer;

impl Timer for SEL2Timer {
    const INTERRUPT_ID: IntId = IntId::ppi(4);

    type TimerInterface = SecureEl2PhysicalTimer;

    unsafe fn timer() -> GenericTimer<Self::TimerInterface> {
        // SAFETY: the caller ensures that no other SecureEl2PhysicalTimer exists and that there is
        // no concurrent access to the CNTHPS_* system registers.
        let timer = unsafe { SecureEl2PhysicalTimer::new() };
        GenericTimer::new(timer)
    }
}
