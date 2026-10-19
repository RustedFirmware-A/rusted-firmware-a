// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S2POE` EL1 context

use arm_sysregs::el1::{
    accessors::{read_s2por_el1, write_s2por_el1},
    registers::S2porEl1,
};

/// `FEAT_S2POE` EL1 context
pub struct S2PoeContext {
    s2por_el1: S2porEl1,
}

impl S2PoeContext {
    pub const EMPTY: Self = Self {
        s2por_el1: S2porEl1::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.s2por_el1 = read_s2por_el1();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_s2por_el1(self.s2por_el1);
        }
    }
}
