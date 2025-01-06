// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "fvp")]
mod fvp;
#[cfg(platform = "qemu")]
mod qemu;
#[cfg(test)]
mod test;

#[cfg(platform = "fvp")]
pub use fvp::Fvp as PlatformImpl;
#[cfg(not(test))]
pub use percore::exception_free;
#[cfg(platform = "qemu")]
pub use qemu::Qemu as PlatformImpl;
#[cfg(test)]
pub use test::{exception_free, TestPlatform as PlatformImpl};

use crate::{context::EntryPointInfo, pagetable::IdMap, services::arch::WorkaroundSupport};
use percore::Cores;

/// The hooks implemented by all platforms.
pub trait Platform: Cores {
    /// The number of CPU cores.
    const CORE_COUNT: usize;

    /// The number of pages to reserve for the page heap.
    const PAGE_HEAP_PAGE_COUNT: usize = 5;

    /// Initialises the logger and anything else the platform needs. This will be called before the
    /// MMU is enabled.
    ///
    /// Any logs sent before this is called will be ignored.
    fn init_beforemmu();

    /// Maps device memory and any other regions specific to the platform, before the MMU is
    /// enabled.
    fn map_extra_regions(idmap: &mut IdMap);

    /// Returns the entry point for the secure world, i.e. BL32.
    fn secure_entry_point() -> EntryPointInfo;

    /// Returns the entry point for the non-secure world, i.e. BL33.
    fn non_secure_entry_point() -> EntryPointInfo;

    /// Powers off the system.
    fn system_off() -> !;

    /// Returns whether this platform supports the arch WORKAROUND_1 SMC.
    fn arch_workaround_1_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_1 SMC.
    fn arch_workaround_1();

    /// Returns whether this platform supports the arch WORKAROUND_2 SMC.
    fn arch_workaround_2_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_2 SMC.
    fn arch_workaround_2();

    /// Returns whether this platform supports the arch WORKAROUND_3 SMC.
    fn arch_workaround_3_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_3 SMC.
    fn arch_workaround_3();

    /// Returns whether this platform supports the arch WORKAROUND_4 SMC.
    fn arch_workaround_4_supported() -> WorkaroundSupport;
}
