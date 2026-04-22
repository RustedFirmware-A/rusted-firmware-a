// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! AArch64 assembly instructions.

#[cfg(all(target_arch = "aarch64", not(test)))]
use core::arch::asm;

/// Issues a full system (`sy`) data synchronization barrier (`dsb`) instruction.
pub fn dsb_sy() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("dsb sy", options(nostack));
    }
}

/// Issues a data synchronization barrier (`dsb`) instruction that applies to the inner shareable
/// domain (`ish`).
#[allow(unused)]
pub fn dsb_ish() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("dsb ish", options(nostack));
    }
}

/// Issues an instruction synchronization barrier (`isb`) instruction.
pub fn isb() {
    // SAFETY: `isb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("isb", options(nostack));
    }
}

/// Causes an event to be signaled to all cores within a multiprocessor system.
#[allow(unused)]
pub fn sev() {
    // SAFETY: `sev` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("sev", options(nostack));
    }
}

/// Issues a translation lookaside buffer invalidate (`tlbi`) instruction that invalidates all TLB
/// entries for EL3 (`alle3`).
pub fn tlbi_alle3() {
    // SAFETY: `tlbi` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("tlbi alle3", options(nostack));
    }
}

/// Wait For Interrupt is a hint instruction that indicates that the PE can enter a low-power state
/// and remain there until a wakeup event occurs.
pub fn wfi() {
    // SAFETY: `wfi` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("wfi", options(nostack));
    }
}

/// Issues a translation lookaside buffer invalidate (`tlbi`) instruction that invalidates
/// cached copies of GPT entries from TLBs. The invalidation affects all TLBs in the
/// Outer Shareable domain.
#[cfg(feature = "rme")]
pub fn tlbi_paallos() {
    // SAFETY: TLB/Cache invalidation does not violate Rust safety.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("sys #6, c8, c1, #4")
    }
}
