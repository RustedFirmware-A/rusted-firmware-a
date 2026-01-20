// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 context management for when Secure EL2 is enabled.

use super::Tcr2;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{Tcr2El2, read_tcr2_el2, write_tcr2_el2};

pub struct Tcr2CpuContext {
    tcr2_el2: Tcr2El2,
}

impl Tcr2CpuContext {
    pub const EMPTY: Self = Self {
        tcr2_el2: Tcr2El2::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Tcr2<CORE_COUNT, PlatformImpl> {
    /// Saves the system register values to this context struct.
    pub fn save_registers(&self, world: World) {
        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx[world].tcr2_el2 = read_tcr2_el2();
        })
    }

    /// Restores the system register values from this context struct.
    pub fn restore_registers(&self, world: World) {
        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            // SAFETY: We're restoring the value previously saved, so it must be valid.
            unsafe {
                write_tcr2_el2(ctx[world].tcr2_el2);
            }
        })
    }
}
