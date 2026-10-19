// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! `FEAT_S1PIE` EL2 context

use arm_sysregs::el2::{
    accessors::{read_pir_el2, read_pire0_el2, write_pir_el2, write_pire0_el2},
    registers::{PirEl2, Pire0El2},
};

/// `FEAT_S1PIE` EL2 context
pub struct S1PieContextEl2 {
    pire0_el2: Pire0El2,
    pir_el2: PirEl2,
}

impl S1PieContextEl2 {
    pub const EMPTY: Self = Self {
        pire0_el2: Pire0El2::empty(),
        pir_el2: PirEl2::empty(),
    };

    /// Saves system registers into the context structure.
    pub fn save(&mut self) {
        self.pire0_el2 = read_pire0_el2();
        self.pir_el2 = read_pir_el2();
    }

    /// Restores system registers from the context structure.
    pub fn restore(&self) {
        // SAFETY: The registers don't have a side effect in EL3.
        unsafe {
            write_pire0_el2(self.pire0_el2);
            write_pir_el2(self.pir_el2);
        }
    }
}
