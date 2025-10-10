// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Cpu;
use crate::naked_asm;

pub struct AemGeneric;

/// Safety: The reset handler is implemented as a naked function and does not clobber any registers.
unsafe impl Cpu for AemGeneric {
    const MIDR: u64 = 0x410f_d0f0;

    #[unsafe(naked)]
    extern "C" fn reset_handler() {
        naked_asm!("ret");
    }

    fn power_down_level0() {}

    fn power_down_level1() {}
}
