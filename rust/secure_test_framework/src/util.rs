// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::arch::asm;

/// The partition ID of the normal world component.
pub const NORMAL_WORLD_ID: u16 = 0;

/// The partition ID of the secure world component.
pub const SECURE_WORLD_ID: u16 = 1;

/// Value returned in a direct message response for a test success.
pub const TEST_SUCCESS: u64 = 0;

/// Value returned in a direct message response for a test failure.
pub const TEST_FAILURE: u64 = 1;

/// Value returned in a direct message response for a test panic. No further tests should be run
/// after this.
pub const TEST_PANIC: u64 = 2;

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
