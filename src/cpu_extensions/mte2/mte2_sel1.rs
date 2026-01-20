// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_MTE2 context management for when Secure EL2 is not enabled.

use super::MemoryTagging;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{
    GcrEl1, RgsrEl1, TfsrEl1, Tfsre0El1, read_gcr_el1, read_rgsr_el1, read_tfsr_el1,
    read_tfsre0_el1, write_gcr_el1, write_rgsr_el1, write_tfsr_el1, write_tfsre0_el1,
};

pub struct Mte2CpuContext {
    tfsre0_el1: Tfsre0El1,
    tfsr_el1: TfsrEl1,
    rgsr_el1: RgsrEl1,
    gcr_el1: GcrEl1,
}

impl Mte2CpuContext {
    pub const EMPTY: Self = Self {
        tfsre0_el1: Tfsre0El1::empty(),
        tfsr_el1: TfsrEl1::empty(),
        rgsr_el1: RgsrEl1::empty(),
        gcr_el1: GcrEl1::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> MemoryTagging<CORE_COUNT, PlatformImpl> {
    pub(crate) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx[world].tfsre0_el1 = read_tfsre0_el1();
            ctx[world].tfsr_el1 = read_tfsr_el1();
            ctx[world].rgsr_el1 = read_rgsr_el1();
            ctx[world].gcr_el1 = read_gcr_el1();
        })
    }

    pub(crate) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            // SAFETY: We're restoring the values previously saved, so they must be valid.
            unsafe {
                write_tfsre0_el1(ctx[world].tfsre0_el1);
                write_tfsr_el1(ctx[world].tfsr_el1);
                write_rgsr_el1(ctx[world].rgsr_el1);
                write_gcr_el1(ctx[world].gcr_el1);
            }
        })
    }
}
