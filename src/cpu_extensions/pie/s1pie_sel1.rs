// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S1PIE` EL1 context

use arm_sysregs::el1::{
    accessors::{read_pir_el1, read_pire0_el1, write_pir_el1, write_pire0_el1},
    registers::{PirEl1, Pire0El1},
};

/// `FEAT_S1PIE` EL1 context
pub struct S1PieContextEl1 {
    pire0_el1: Pire0El1,
    pir_el1: PirEl1,
}

impl S1PieContextEl1 {
    pub const EMPTY: Self = Self {
        pire0_el1: Pire0El1::empty(),
        pir_el1: PirEl1::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.pire0_el1 = read_pire0_el1();
        self.pir_el1 = read_pir_el1();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_pire0_el1(self.pire0_el1);
            write_pir_el1(self.pir_el1);
        }
    }
}
