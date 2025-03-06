// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(target_arch = "aarch64")]
use core::arch::asm;

/// Issues a data synchronization barrier (`dsb`) instruction that applies to the inner shareable
/// domain (`ish`).
pub fn dsb_ish() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("dsb ish", options(nostack));
    }
}

/// Issues an instruction synchronization barrier (`isb`) instruction.
pub fn isb() {
    // SAFETY: `isb` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("isb", options(nostack));
    }
}

/// Issues a translation lookaside buffer invalidate (`tlbi`) instruction that invalidates all TLB
/// entries for EL3 (`alle3`).
pub fn tlbi_alle3() {
    // SAFETY: `tlbi` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("tlbi alle3", options(nostack));
    }
}
