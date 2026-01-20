// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_RAS context management for when Secure EL2 is not enabled.

use super::Ras;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{DisrEl1, read_disr_el1, write_disr_el1};

pub struct RasCpuContext {
    disr_el1: DisrEl1,
}

impl RasCpuContext {
    pub const EMPTY: Self = Self {
        disr_el1: DisrEl1::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Ras<CORE_COUNT, PlatformImpl> {
    /// Saves the system register values to this context struct.
    pub(crate) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            self.context.get().borrow_mut(token)[world].disr_el1 = read_disr_el1();
        })
    }

    /// Restores the system register values from this context struct.
    pub(crate) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            write_disr_el1(self.context.get().borrow_mut(token)[world].disr_el1);
        })
    }
}
