// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![no_main]
#![no_std]
#![warn(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]

mod context;
mod exceptions;
mod layout;
mod logger;
mod pagetable;
mod pl011;
mod platform;
mod semihosting;
mod sysregs;

use crate::platform::{Platform, PlatformImpl};
use buddy_system_allocator::LockedHeap;
use log::info;

const HEAP_SIZE: usize = 20 * 1024;

static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::<32>::new();

#[no_mangle]
extern "C" fn bl31_main(bl31_params: u64, platform_params: u64) {
    PlatformImpl::init_beforemmu();
    info!("Rust BL31 starting");
    info!("Parameters: {:#0x} {:#0x}", bl31_params, platform_params);

    // Initialise heap.
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP.as_mut_ptr() as usize, HEAP.len());
    }

    // Set up page tables.
    let idmap = pagetable::init();

    info!("Page table activated.");

    loop {}
}
