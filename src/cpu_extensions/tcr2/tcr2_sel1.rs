// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 context management for when Secure EL2 is not enabled.

use super::{TCR2_CONTEXT, Tcr2};
use crate::{context::World, platform::exception_free};
use arm_sysregs::el1::{
    accessors::{read_tcr2_el1, write_tcr2_el1},
    registers::Tcr2El1,
};

pub struct Tcr2CpuContext {
    tcr2_el1: Tcr2El1,
}

impl Tcr2CpuContext {
    pub const EMPTY: Self = Self {
        tcr2_el1: Tcr2El1::empty(),
    };
}

impl Tcr2 {
    /// Saves the system register values to this context struct.
    pub fn save_registers(&self, world: World) {
        exception_free(|token| {
            TCR2_CONTEXT.get().borrow_mut(token)[world].tcr2_el1 = read_tcr2_el1();
        })
    }

    /// Restores the system register values from this context struct.
    pub fn restore_registers(&self, world: World) {
        exception_free(|token| {
            // SAFETY: We're restoring the value previously saved, so it must be valid.
            unsafe {
                write_tcr2_el1(TCR2_CONTEXT.get().borrow_mut(token)[world].tcr2_el1);
            }
        })
    }
}
