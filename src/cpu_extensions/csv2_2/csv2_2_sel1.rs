// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Cache Speculation Variant 2 Version 2, EL1 context management

use super::{CSV2_2CONTEXT, Csv2_2};
use crate::{context::World, platform::exception_free};
use arm_sysregs::{
    el0::{
        accessors::{read_scxtnum_el0, write_scxtnum_el0},
        registers::ScxtnumEl0,
    },
    el1::{
        accessors::{read_scxtnum_el1, write_scxtnum_el1},
        registers::ScxtnumEl1,
    },
};

pub struct Csv2_2ContextSel1 {
    scxtnum_el0: ScxtnumEl0,
    scxtnum_el1: ScxtnumEl1,
}

impl Csv2_2ContextSel1 {
    pub const EMPTY: Self = Self {
        scxtnum_el0: ScxtnumEl0::empty(),
        scxtnum_el1: ScxtnumEl1::empty(),
    };
}

impl Csv2_2 {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el0 = read_scxtnum_el0();
            CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el1 = read_scxtnum_el1();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_CSV2_2 is assumed to be present, and the saved register values
            // are assumed to be valid.
            unsafe {
                write_scxtnum_el0(CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el0);
                write_scxtnum_el1(CSV2_2CONTEXT.get().borrow_mut(token)[world].scxtnum_el1);
            }
        });
    }
}
