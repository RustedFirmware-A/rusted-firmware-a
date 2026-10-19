// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S1POE` EL1 context

use arm_sysregs::el1::{
    accessors::{read_por_el1, write_por_el1},
    registers::PorEl1,
};

/// `FEAT_S1POE` EL1 context
pub struct S1PoeContextEl1 {
    por_el1: PorEl1,
}

impl S1PoeContextEl1 {
    pub const EMPTY: Self = Self {
        por_el1: PorEl1::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.por_el1 = read_por_el1();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_por_el1(self.por_el1);
        }
    }
}
