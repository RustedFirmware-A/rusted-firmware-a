// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RF-A: A new implementation of TF-A for AArch64.

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]

mod aarch64;
mod context;
#[cfg(not(test))]
mod crash_console;
mod debug;
mod exceptions;
mod gicv3;
#[cfg_attr(test, path = "layout_fake.rs")]
mod layout;
mod logger;
mod pagetable;
mod platform;
#[cfg(platform = "qemu")]
mod semihosting;
mod services;
mod smccc;
#[cfg(not(test))]
mod stacks;
mod sysregs;

use crate::platform::{Platform, PlatformImpl};
use context::initialise_contexts;
use log::info;
use services::Services;

#[unsafe(no_mangle)]
extern "C" fn bl31_main(bl31_params: u64, platform_params: u64) -> ! {
    PlatformImpl::init_before_mmu();
    info!("Rust BL31 starting");
    info!("Parameters: {:#0x} {:#0x}", bl31_params, platform_params);

    // Set up page table.
    pagetable::init();
    info!("Page table activated.");

    // Set up GIC.
    gicv3::init();
    info!("GIC configured.");

    let non_secure_entry_point = PlatformImpl::non_secure_entry_point();
    let secure_entry_point = PlatformImpl::secure_entry_point();
    #[cfg(feature = "rme")]
    let realm_entry_point = PlatformImpl::realm_entry_point();
    initialise_contexts(
        &non_secure_entry_point,
        &secure_entry_point,
        #[cfg(feature = "rme")]
        &realm_entry_point,
    );

    Services::get().run_loop();
}

#[cfg(target_arch = "aarch64")]
mod asm {
    use crate::{
        debug::{DEBUG, ENABLE_ASSERTIONS},
        sysregs::SctlrEl3,
    };
    use core::arch::global_asm;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("misc_helpers.S"),
        include_str!("asm_macros_common_purge.S"),
        ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
        DEBUG = const DEBUG as i32,
        SCTLR_M_BIT = const SctlrEl3::M.bits(),
    );
}
