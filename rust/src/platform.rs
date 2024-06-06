// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(feature = "plat-qemu")]
mod qemu;
use aarch64_paging::idmap::IdMap;
#[cfg(feature = "plat-qemu")]
pub use qemu::{Qemu as PlatformImpl, BL31_BASE};

/// The hooks implemented by all platforms.
pub trait Platform {
    /// The number of CPU cores.
    const CORE_COUNT: usize;

    /// Initialises the logger and anything else the platform needs. This will be called before the
    /// MMU is enabled.
    ///
    /// Any logs sent before this is called will be ignored.
    fn init_beforemmu();

    /// Maps device memory and any other regions specific to the platform, before the MMU is
    /// enabled.
    fn map_extra_regions(idmap: &mut IdMap);
}
