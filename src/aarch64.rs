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
pub fn dsb_ish() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("dsb ish", options(nostack));
    }
}

/// Issues a data synchronization barrier (`dsb`) instruction that applies to the outer shareable
/// domain (`osh`), for read and write accesses.
pub fn dsb_osh() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("dsb osh", options(nostack));
    }
}

/// Issues a data synchronization barrier (`dsb`) instruction that applies to the outer shareable
/// domain (`osh`), for write accesses (st).
pub fn dsb_oshst() {
    // SAFETY: `dsb` does not violate safe Rust guarantees.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("dsb oshst", options(nostack));
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

/// Supported sizes for [`tlbi_rpalos`].
#[cfg(feature = "rme")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub enum TlbiSize {
    /// 4 KB.
    KB4 = 0b0000,
    /// 16 KB.
    KB16 = 0b0001,
    /// 64 KB.
    KB64 = 0b0010,
    /// 2 MB.
    MB2 = 0b0011,
    /// 32 MB.
    MB32 = 0b0100,
    /// 512 MB.
    MB512 = 0b0101,
    /// 1 GB.
    GB1 = 0b0110,
    /// 16 GB.
    GB16 = 0b0111,
    /// 64 GB.
    GB64 = 0b1000,
    /// 512 GB.
    GB512 = 0b1001,
}
#[cfg(feature = "rme")]
const TLBI_ADDR_SHIFT: usize = 12;
#[cfg(feature = "rme")]
const TLBI_SIZE_SHIFT: usize = 44;

#[cfg(feature = "rme")]
/// Issues a `tlbi_rpalos` instruction (TLB Range Invalidate GPT Information by PA, Last level, Outer Shareable).
/// Only valid on systems with RME, otherwise undefined.
pub fn tlbi_rpalos(addr: usize, size: TlbiSize) {
    let arg: usize = (addr >> TLBI_ADDR_SHIFT) | ((size as usize) << TLBI_SIZE_SHIFT);
    // Safety: TLB/Cache invalidation does not violate Rust safety.
    #[cfg(all(target_arch = "aarch64", not(test)))]
    unsafe {
        asm!("sys #6, c8, c4, #7, {0}" , in(reg) arg)
    };
    let _ = arg;
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
