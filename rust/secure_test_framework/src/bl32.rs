// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod expect;
mod ffa;
mod logger;
mod platform;
mod secure_tests;
mod util;

use crate::{
    ffa::direct_response,
    ffa::msg_wait,
    platform::{Platform, PlatformImpl},
    secure_tests::run_test,
    util::{current_el, NORMAL_WORLD_ID, SECURE_WORLD_ID, TEST_FAILURE, TEST_PANIC, TEST_SUCCESS},
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, Interface};
use core::panic::PanicInfo;
use log::{error, info, LevelFilter};

entry!(bl32_main, 4);
fn bl32_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink, LevelFilter::Trace).unwrap();

    info!(
        "Rust BL32 starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        x0,
        x1,
        x2,
        x3,
    );

    // Wait for the first test index.
    let mut message = msg_wait(None).unwrap();

    loop {
        let Interface::MsgSendDirectReq {
            src_id,
            dst_id,
            args,
        } = message
        else {
            panic!("Unexpected FF-A interface returned: {:?}", message);
        };
        assert_eq!(src_id, NORMAL_WORLD_ID);
        assert_eq!(dst_id, SECURE_WORLD_ID);

        let DirectMsgArgs::Args64(args) = args else {
            panic!("Received unexpected direct message type.");
        };
        let test_index = args[0];

        let test_result = if run_test(test_index).is_ok() {
            TEST_SUCCESS
        } else {
            TEST_FAILURE
        };

        // Return result and wait for the next test index.
        message = direct_response(
            SECURE_WORLD_ID,
            NORMAL_WORLD_ID,
            DirectMsgArgs::Args64([test_result, 0, 0, 0, 0]),
        )
        .unwrap();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Log the panic message
    error!("{}", info);
    // Tell normal world that the test failed.
    let _ = direct_response(
        SECURE_WORLD_ID,
        NORMAL_WORLD_ID,
        DirectMsgArgs::Args64([TEST_PANIC, 0, 0, 0, 0]),
    );
    loop {}
}
