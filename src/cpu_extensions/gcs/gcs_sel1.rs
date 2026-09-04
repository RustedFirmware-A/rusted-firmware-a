// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{GCS_CONTEXT, Gcs};
use crate::{context::World, platform::exception_free};
use arm_sysregs::{
    el0::{
        accessors::{read_gcspr_el0, write_gcspr_el0},
        registers::GcsprEl0,
    },
    el1::{
        accessors::{
            read_gcscr_el1, read_gcscre0_el1, read_gcspr_el1, write_gcscr_el1, write_gcscre0_el1,
            write_gcspr_el1,
        },
        registers::{GcscrEl1, Gcscre0El1, GcsprEl1},
    },
};

pub struct GcsContextEl1 {
    gcscre0_el1: Gcscre0El1,
    gcscr_el1: GcscrEl1,
    gcspr_el0: GcsprEl0,
    gcspr_el1: GcsprEl1,
}

impl GcsContextEl1 {
    pub const EMPTY: Self = Self {
        gcscre0_el1: Gcscre0El1::empty(),
        gcscr_el1: GcscrEl1::empty(),
        gcspr_el0: GcsprEl0::empty(),
        gcspr_el1: GcsprEl1::empty(),
    };
}

impl Gcs {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            GCS_CONTEXT.get().borrow_mut(token)[world].gcscre0_el1 = read_gcscre0_el1();
            GCS_CONTEXT.get().borrow_mut(token)[world].gcscr_el1 = read_gcscr_el1();
            GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el0 = read_gcspr_el0();
            GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el1 = read_gcspr_el1();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_GCS is assumed to be present, and the saved values
            // are assumed to be valid.
            unsafe {
                write_gcscre0_el1(GCS_CONTEXT.get().borrow_mut(token)[world].gcscre0_el1);
                write_gcscr_el1(GCS_CONTEXT.get().borrow_mut(token)[world].gcscr_el1);
                write_gcspr_el0(GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el0);
                write_gcspr_el1(GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el1);
            }
        });
    }
}
