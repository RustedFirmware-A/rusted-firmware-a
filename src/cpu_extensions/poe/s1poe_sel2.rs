// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S1POE` EL2 context

use arm_sysregs::el2::{
    accessors::{read_por_el2, write_por_el2},
    registers::PorEl2,
};

/// `FEAT_S1POE` EL2 context
pub struct S1PoeContextEl2 {
    por_el2: PorEl2,
}

impl S1PoeContextEl2 {
    pub const EMPTY: Self = Self {
        por_el2: PorEl2::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.por_el2 = read_por_el2();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_por_el2(self.por_el2);
        }
    }
}
