// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::arch::asm;

/// Returns the current exception level at which we are running.
pub fn current_el() -> u8 {
    let current_el: u64;
    // SAFETY: Reading `CurrentEL` is always safe, it has no impact on memory.
    unsafe {
        asm!(
            "mrs {current_el}, CurrentEL",
            options(nostack),
            current_el = out(reg) current_el,
        );
    }
    (current_el >> 2) as u8
}
