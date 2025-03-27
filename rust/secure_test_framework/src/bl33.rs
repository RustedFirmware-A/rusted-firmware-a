// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod logger;
mod platform;
mod util;

use crate::{
    platform::{Platform, PlatformImpl},
    util::current_el,
};
use aarch64_rt::entry;
use core::panic::PanicInfo;
use log::{error, info, LevelFilter};
use smccc::{psci, Smc};

entry!(bl33_main, 4);
fn bl33_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink, LevelFilter::Trace).unwrap();

    info!(
        "Rust BL33 starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        x0,
        x1,
        x2,
        x3,
    );

    let ret = psci::system_off::<Smc>();
    panic!("PSCI_SYSTEM_OFF returned {:?}", ret);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    loop {}
}
