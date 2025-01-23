// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]
#![warn(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

mod context;
mod exceptions;
#[cfg_attr(test, path = "layout_fake.rs")]
mod layout;
mod logger;
mod pagetable;
mod platform;
#[cfg(platform = "qemu")]
mod semihosting;
mod services;
mod smccc;
mod sysregs;

use crate::platform::{Platform, PlatformImpl};
use context::{initialise_contexts, set_initial_world, World};
use log::info;

#[unsafe(no_mangle)]
extern "C" fn bl31_main(bl31_params: u64, platform_params: u64) {
    PlatformImpl::init_beforemmu();
    info!("Rust BL31 starting");
    info!("Parameters: {:#0x} {:#0x}", bl31_params, platform_params);

    // Set up page table.
    pagetable::init();
    info!("Page table activated.");

    let non_secure_entry_point = PlatformImpl::non_secure_entry_point();
    let secure_entry_point = PlatformImpl::secure_entry_point();
    initialise_contexts(&non_secure_entry_point, &secure_entry_point);
    set_initial_world(World::NonSecure);
    info!("Entering next stage...");
}
