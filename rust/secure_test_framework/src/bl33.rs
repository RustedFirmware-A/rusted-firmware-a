// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod exceptions;
mod expect;
mod ffa;
mod logger;
mod normal_world_tests;
mod platform;
mod util;

use crate::{
    exceptions::set_exception_vector,
    ffa::direct_request,
    normal_world_tests::{NORMAL_TEST_COUNT, run_test},
    platform::{Platform, PlatformImpl},
    util::{NORMAL_WORLD_ID, SECURE_WORLD_ID, TEST_FAILURE, TEST_PANIC, TEST_SUCCESS, current_el},
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, Interface};
use core::panic::PanicInfo;
use log::{LevelFilter, error, info, warn};
use smccc::{Smc, psci};

/// The version of FF-A which we support.
const FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0);

/// An unreasonably high FF-A version number.
const HIGH_FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0xffff);

/// The number of tests in the BL32 component of STF.
const SECURE_TEST_COUNT: u64 = 2;

entry!(bl33_main, 4);
fn bl33_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink, LevelFilter::Trace).unwrap();

    set_exception_vector();

    info!(
        "Rust BL33 starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        x0,
        x1,
        x2,
        x3,
    );

    // Test what happens if we try a much higher version.
    let spmc_supported_ffa_version = ffa::version(HIGH_FFA_VERSION).expect("FFA_VERSION failed");
    info!("SPMC supports FF-A version {}", spmc_supported_ffa_version);
    assert!(spmc_supported_ffa_version >= FFA_VERSION);
    assert!(spmc_supported_ffa_version < HIGH_FFA_VERSION);
    // Negotiate the FF-A version we actually support. This must happen before any other FF-A calls.
    assert_eq!(ffa::version(FFA_VERSION), Ok(FFA_VERSION));

    // Run normal world tests.
    let mut passing_normal_test_count = 0;
    for test_index in 0..NORMAL_TEST_COUNT {
        if run_test(test_index).is_ok() {
            info!("Normal world test {} passed", test_index);
            passing_normal_test_count += 1;
        } else {
            warn!("Normal world test {} failed", test_index);
        }
    }
    info!(
        "{}/{} tests passed in normal world",
        passing_normal_test_count, NORMAL_TEST_COUNT
    );

    // Run secure world tests.
    let mut passing_secure_test_count = 0;
    for test_index in 0..SECURE_TEST_COUNT {
        info!("Requesting secure world test {} run...", test_index);
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
                    info!("Secure world test {} passed", test_index);
                    passing_secure_test_count += 1;
                }
                DirectMsgArgs::Args64([TEST_FAILURE, ..]) => {
                    warn!("Secure world test {} failed", test_index);
                }
                DirectMsgArgs::Args64([TEST_PANIC, ..]) => {
                    warn!("Secure world test {} panicked", test_index);
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
        passing_secure_test_count, SECURE_TEST_COUNT
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
