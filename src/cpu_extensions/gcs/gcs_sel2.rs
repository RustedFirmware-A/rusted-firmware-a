// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{GCS_CONTEXT, Gcs};
use crate::{context::World, platform::exception_free};
use arm_sysregs::el2::{
    accessors::{read_gcscr_el2, read_gcspr_el2, write_gcscr_el2, write_gcspr_el2},
    registers::{GcscrEl2, GcsprEl2},
};

pub struct GcsContextEl2 {
    gcscr_el2: GcscrEl2,
    gcspr_el2: GcsprEl2,
}

impl GcsContextEl2 {
    pub const EMPTY: Self = Self {
        gcscr_el2: GcscrEl2::empty(),
        gcspr_el2: GcsprEl2::empty(),
    };
}

impl Gcs {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            GCS_CONTEXT.get().borrow_mut(token)[world].gcscr_el2 = read_gcscr_el2();
            GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el2 = read_gcspr_el2();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_GCS is assumed to be present, and the saved values
            // are assumed to be valid.
            unsafe {
                write_gcscr_el2(GCS_CONTEXT.get().borrow_mut(token)[world].gcscr_el2);
                write_gcspr_el2(GCS_CONTEXT.get().borrow_mut(token)[world].gcspr_el2);
            }
        });
    }
}
