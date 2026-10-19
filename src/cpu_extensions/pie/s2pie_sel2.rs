// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S2PIE` EL2 context

use arm_sysregs::el2::{
    accessors::{read_s2pir_el2, write_s2pir_el2},
    registers::S2pirEl2,
};

/// `FEAT_S2PIE` EL2 context
pub struct S2PieContext {
    s2pir_el2: S2pirEl2,
}

impl S2PieContext {
    pub const EMPTY: Self = Self {
        s2pir_el2: S2pirEl2::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.s2pir_el2 = read_s2pir_el2();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_s2pir_el2(self.s2pir_el2);
        }
    }
}
