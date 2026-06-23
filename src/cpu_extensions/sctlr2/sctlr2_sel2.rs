// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_SCTLR2 context management for when Secure EL2 is enabled.

use super::Sctlr2;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{Sctlr2El2, read_sctlr2_el2, write_sctlr2_el2};

pub struct Sctlr2CpuContext {
    sctlr2_el2: Sctlr2El2,
}

impl Sctlr2CpuContext {
    pub const EMPTY: Self = Self {
        sctlr2_el2: Sctlr2El2::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Sctlr2<CORE_COUNT, PlatformImpl> {
    pub(super) fn save_registers(&self, world: World) {
        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx[world].sctlr2_el2 = read_sctlr2_el2();
        })
    }

    pub(super) fn restore_registers(&self, world: World) {
        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            // SAFETY: We're restoring the value previously saved, so it must be valid.
            unsafe {
                write_sctlr2_el2(ctx[world].sctlr2_el2);
            }
        })
    }
}
