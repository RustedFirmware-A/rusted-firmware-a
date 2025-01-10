// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod ffa;
mod logger;
mod platform;
mod util;

use crate::{
    ffa::direct_request,
    platform::{Platform, PlatformImpl},
    util::{current_el, NORMAL_WORLD_ID, SECURE_WORLD_ID, TEST_FAILURE, TEST_PANIC, TEST_SUCCESS},
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, Interface};
use core::panic::PanicInfo;
use log::{error, info, warn, LevelFilter};
use smccc::{psci, Smc};

const TEST_COUNT: u64 = 2;

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

    let mut passing_test_count = 0;
    for test_index in 0..TEST_COUNT {
        info!("Requesting test {} run...", test_index);
        let result = direct_request(
            NORMAL_WORLD_ID,
            SECURE_WORLD_ID,
            DirectMsgArgs::Args64([test_index, 0, 0, 0, 0]),
        )
        .expect("Failed to parse direct request response");
        if let Interface::MsgSendDirectResp {
            src_id,
            dst_id,
            args,
        } = result
        {
            assert_eq!(src_id, SECURE_WORLD_ID);
            assert_eq!(dst_id, NORMAL_WORLD_ID);
            match args {
                DirectMsgArgs::Args64([TEST_SUCCESS, ..]) => {
                    info!("Test {} passed", test_index);
                    passing_test_count += 1;
                }
                DirectMsgArgs::Args64([TEST_FAILURE, ..]) => {
                    warn!("Test {} failed", test_index);
                }
                DirectMsgArgs::Args64([TEST_PANIC, ..]) => {
                    warn!("Test {} panicked", test_index);
                    // We can't continue running other tests after one panics.
                    break;
                }
                _ => {
                    warn!("Unexpected direct message response: {:?}", args);
                }
            }
        } else {
            warn!("Unexpected response {:?}", result);
        }
    }
    info!(
        "{}/{} tests passed in secure world",
        passing_test_count, TEST_COUNT
    );

    let ret = psci::system_off::<Smc>();
    panic!("PSCI_SYSTEM_OFF returned {:?}", ret);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    let _ = psci::system_off::<Smc>();
    loop {}
}
