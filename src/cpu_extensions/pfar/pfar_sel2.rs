// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Physical Fault Address Register Extension, EL2 context management

use super::Pfar;
use crate::{
    context::World,
    platform::{Platform, exception_free},
};
use arm_sysregs::{PfarEl2, read_pfar_el2, write_pfar_el2};

pub struct PfarContext {
    pfar_el2: PfarEl2,
}

impl PfarContext {
    pub const EMPTY: Self = Self {
        pfar_el2: PfarEl2::empty(),
    };
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Pfar<CORE_COUNT, PlatformImpl> {
    pub(super) fn save_context_internal(&self, world: World) {
        exception_free(|token| {
            self.context.get().borrow_mut(token)[world].pfar_el2 = read_pfar_el2();
        });
    }

    pub(super) fn restore_context_internal(&self, world: World) {
        exception_free(|token| {
            // SAFETY: FEAT_PFAR is assumed to be present, and the saved `pfar_el2` value is
            // assumed to be valid.
            unsafe {
                write_pfar_el2(self.context.get().borrow_mut(token)[world].pfar_el2);
            }
        });
    }
}
