// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_HCX context management for when Secure EL2 is enabled.

use super::{HCX_CONTEXT, Hcx};
use crate::{context::World, platform::exception_free};
use arm_sysregs::{HcrxEl2, read_hcrx_el2, write_hcrx_el2};

pub struct HcxCpuContext {
    hcrx_el2: HcrxEl2,
}

impl HcxCpuContext {
    pub const EMPTY: Self = Self {
        hcrx_el2: HcrxEl2::empty(),
    };
}

impl Hcx {
    /// Saves the system register values to this context struct.
    pub fn save_el2_context(&self, world: World) {
        exception_free(|token| {
            HCX_CONTEXT.get().borrow_mut(token)[world].hcrx_el2 = read_hcrx_el2();
        })
    }

    /// Restores the system register values from this context struct.
    pub fn restore_el2_context(&self, world: World) {
        exception_free(|token| {
            // SAFETY: We're restoring the value previously saved, so it must be valid.
            unsafe {
                write_hcrx_el2(HCX_CONTEXT.get().borrow_mut(token)[world].hcrx_el2);
            }
        })
    }
}
