// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_MTE2 context management for when Secure EL2 is enabled.

use super::MemoryTagging;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{TfsrEl2, read_tfsr_el2, write_tfsr_el2};

pub struct Mte2CpuContext {
    tfsr_el2: TfsrEl2,
}

impl Mte2CpuContext {
    pub const EMPTY: Self = Self {
        tfsr_el2: TfsrEl2::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> MemoryTagging<CORE_COUNT, PlatformImpl> {
    /// Saves the system register values to this context struct.
    pub(crate) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx[world].tfsr_el2 = read_tfsr_el2();
        })
    }

    /// Restores the system register values from this context struct.
    pub(crate) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            write_tfsr_el2(ctx[world].tfsr_el2);
        })
    }
}
