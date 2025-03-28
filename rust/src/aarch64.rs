// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(target_arch = "aarch64")]
use core::arch::asm;

/// Issues a full system (`sy`) data synchronization barrier (`dsb`) instruction.
pub fn dsb_sy() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("dsb sy", options(nostack));
    }
}

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

/// Causes an event to be signaled to all cores within a multiprocessor system.
pub fn sev() {
    // SAFETY: `sev` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("sev");
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

/// Wait For Interrupt is a hint instruction that indicates that the PE can enter a low-power state
/// and remain there until a wakeup event occurs.
pub fn wfi() {
    // SAFETY: `wfi` does not violate safe Rust guarantees.
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!("wfi", options(nostack));
    }
}
