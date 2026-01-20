// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_RAS context management for when Secure EL2 is enabled.

use super::Ras;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{
    VdisrEl2, VsesrEl2, read_vdisr_el2, read_vsesr_el2, write_vdisr_el2, write_vsesr_el2,
};

pub struct RasCpuContext {
    vdisr_el2: VdisrEl2,
    vsesr_el2: VsesrEl2,
}

impl RasCpuContext {
    pub const EMPTY: Self = Self {
        vdisr_el2: VdisrEl2::empty(),
        vsesr_el2: VsesrEl2::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Ras<CORE_COUNT, PlatformImpl> {
    /// Saves the system register values to this context struct.
    pub(crate) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx[world].vdisr_el2 = read_vdisr_el2();
            ctx[world].vsesr_el2 = read_vsesr_el2();
        })
    }

    /// Restores the system register values from this context struct.
    pub(crate) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            write_vdisr_el2(ctx[world].vdisr_el2);
            write_vsesr_el2(ctx[world].vsesr_el2);
        })
    }
}
