// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Cache Speculation Variant 2 Version 2, EL2 context management

use super::{CSV2_2CONTEXT, Csv2_2};
use crate::{context::World, platform::exception_free};
use arm_sysregs::el2::{
    accessors::{read_scxtnum_el2, write_scxtnum_el2},
    registers::ScxtnumEl2,
};

pub struct Csv2_2ContextSel2 {
    scxtnum_el2: ScxtnumEl2,
}

impl Csv2_2ContextSel2 {
    pub const EMPTY: Self = Self {
        scxtnum_el2: ScxtnumEl2::empty(),
    };
}

impl Csv2_2 {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el2 = read_scxtnum_el2();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_CSV2_2 is assumed to be present, and the saved register values
            // are assumed to be valid.
            unsafe {
                write_scxtnum_el2(CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el2);
            }
        });
    }
}
