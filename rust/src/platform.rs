// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "fvp")]
mod fvp;
#[cfg(platform = "qemu")]
mod qemu;

#[cfg(platform = "fvp")]
pub use fvp::{Fvp as PlatformImpl, BL31_BASE};
#[cfg(platform = "qemu")]
pub use qemu::{Qemu as PlatformImpl, BL31_BASE};

use crate::{context::EntryPointInfo, pagetable::IdMap};

/// The hooks implemented by all platforms.
pub trait Platform {
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

    /// Returns the entry point for the non-secure world, i.e. BL33.
    fn non_secure_entry_point() -> EntryPointInfo;
}
