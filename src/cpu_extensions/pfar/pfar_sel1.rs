// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Physical Fault Address Register Extension, EL1 context management

use super::{PFAR_CONTEXT, Pfar};
use crate::{context::World, platform::exception_free};
use arm_sysregs::el1::{
    accessors::{read_pfar_el1, write_pfar_el1},
    registers::PfarEl1,
};

pub struct PfarContext {
    pfar_el1: PfarEl1,
}

impl PfarContext {
    pub const EMPTY: Self = Self {
        pfar_el1: PfarEl1::empty(),
    };
}

impl Pfar {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            PFAR_CONTEXT.get().borrow_mut(token)[world].pfar_el1 = read_pfar_el1();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_PFAR is assumed to be present, and the saved `pfar_el1` value is
            // assumed to be valid.
            unsafe {
                write_pfar_el1(PFAR_CONTEXT.get().borrow_mut(token)[world].pfar_el1);
            }
        });
    }
}
