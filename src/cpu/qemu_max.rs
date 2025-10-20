// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Cpu;
use crate::naked_asm;

pub struct QemuMax;

/// SAFETY: `reset_handler` and `dump_registers` are implemented as naked functions and don't touch
/// any registers.
unsafe impl Cpu for QemuMax {
    const MIDR: u64 = 0x000f_0510;

    #[unsafe(naked)]
    extern "C" fn reset_handler() {
        naked_asm!("ret");
    }

    #[unsafe(naked)]
    extern "C" fn dump_registers() {
        naked_asm!("ret");
    }

    fn power_down_level0() {}

    fn power_down_level1() {}
}
