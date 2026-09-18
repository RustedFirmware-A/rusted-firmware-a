// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Arm Generic Timer components that are used during power management.

use crate::power_management::TimerInterface;
use arm_generic_timer::memory_mapped::{CntAcr, GenericTimerControl, GenericTimerCtl};
use rf_a_core::reexports::spin::mutex::SpinMutex;

/// Arm Generic Timer (MMIO) based `TimerInterface` implementation for `ArmPsciPlatform`.
pub struct GenericMmioTimer<'a> {
    timer_control: SpinMutex<GenericTimerControl<'a>>,
    timer_ctl: SpinMutex<GenericTimerCtl<'a>>,
    timer_index: usize,
}

impl<'a> GenericMmioTimer<'a> {
    /// Creates new instance.
    pub fn new(
        timer_control: SpinMutex<GenericTimerControl<'a>>,
        timer_ctl: SpinMutex<GenericTimerCtl<'a>>,
        timer_index: usize,
    ) -> Self {
        Self {
            timer_control,
            timer_ctl,
            timer_index,
        }
    }
}

impl<'a> TimerInterface for GenericMmioTimer<'a> {
    fn init(&self) {
        let mut timer_control = self.timer_control.lock();

        timer_control.set_enable(true);

        let frequency = timer_control.base_frequency();

        let mut timer_ctl = self.timer_ctl.lock();

        timer_ctl.set_access_control(self.timer_index, CntAcr::all());
        timer_ctl.set_non_secure_access(self.timer_index, true);
        timer_ctl.set_frequency(frequency);
    }

    fn frequency(&self) -> u32 {
        self.timer_control.lock().base_frequency()
    }
}
