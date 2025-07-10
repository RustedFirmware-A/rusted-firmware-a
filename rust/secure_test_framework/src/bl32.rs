// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod exceptions;
mod expect;
mod ffa;
mod gicv3;
mod logger;
mod platform;
mod secure_tests;
mod util;

use crate::{
    exceptions::set_exception_vector,
    ffa::{direct_response, msg_wait},
    gicv3::init,
    platform::{Platform, PlatformImpl},
    secure_tests::run_test,
    util::{
        NORMAL_WORLD_ID, SECURE_WORLD_ID, SPMC_DEFAULT_ID, SPMD_DEFAULT_ID, TEST_FAILURE,
        TEST_PANIC, TEST_SUCCESS, current_el,
    },
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, FfaError, Interface, SuccessArgsIdGet};
use core::panic::PanicInfo;
use log::{LevelFilter, error, info, warn};

/// The version of FF-A which we support.
const FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 1);

/// An unreasonably high FF-A version number.
const HIGH_FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0xffff);

entry!(bl32_main, 4);
fn bl32_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink, LevelFilter::Trace).unwrap();

    set_exception_vector();
    gicv3::init();

    info!(
        "Rust BL32 starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        x0,
        x1,
        x2,
        x3,
    );

    // Test what happens if we try a much higher version.
    let el3_supported_ffa_version = ffa::version(HIGH_FFA_VERSION).expect("FFA_VERSION failed");
    info!("EL3 supports FF-A version {}", el3_supported_ffa_version);
    assert!(el3_supported_ffa_version >= FFA_VERSION);
    assert!(el3_supported_ffa_version < HIGH_FFA_VERSION);
    // Negotiate the FF-A version we actually support. This must happen before any other FF-A calls.
    assert_eq!(ffa::version(FFA_VERSION), Ok(FFA_VERSION));

    let mut nwd_supported_ffa_version;

    let spmc_id = {
        match ffa::id_get().expect("FFA_ID_GET failed") {
            Interface::Success { args, .. } => SuccessArgsIdGet::try_from(args).unwrap().id,
            Interface::Error {
                error_code: FfaError::NotSupported,
                ..
            } => {
                warn!("FFA_ID_GET not supported");
                SPMC_DEFAULT_ID
            }
            res => panic!("Unexpected response for FFA_ID_GET: {:?}", res),
        }
    };

    let spmd_id = {
        match ffa::spm_id_get().expect("FFA_SPM_ID_GET failed") {
            Interface::Success { args, .. } => SuccessArgsIdGet::try_from(args).unwrap().id,
            Interface::Error {
                error_code: FfaError::NotSupported,
                ..
            } => {
                warn!("FFA_SPM_ID_GET not supported");
                SPMD_DEFAULT_ID
            }
            res => panic!("Unexpected response for FFA_SPM_ID_GET: {:?}", res),
        }
    };

    // Wait for the first test index.
    let mut message = msg_wait(None).unwrap();

    loop {
        match message {
            Interface::MsgSendDirectReq {
                src_id,
                dst_id,
                args,
            } => {
                let response_args = if src_id == NORMAL_WORLD_ID && dst_id == SECURE_WORLD_ID {
                    let DirectMsgArgs::Args64(args) = args else {
                        panic!("Received unexpected direct message type from Normal World.");
                    };

                    let test_index = args[0];
                    let test_result = if run_test(test_index).is_ok() {
                        TEST_SUCCESS
                    } else {
                        TEST_FAILURE
                    };

                    DirectMsgArgs::Args64([test_result, 0, 0, 0, 0])
                } else if src_id == spmd_id && dst_id == spmc_id {
                    let DirectMsgArgs::VersionReq { version } = args else {
                        panic!("Received unexpected direct message type from SPMD.");
                    };

                    let out_version = if version.is_compatible_to(&FFA_VERSION) {
                        // If NWd queries a version that we're compatible with, return the same
                        nwd_supported_ffa_version = version;
                        info!(
                            "Normal World supports FF-A version {}",
                            nwd_supported_ffa_version
                        );
                        nwd_supported_ffa_version
                    } else {
                        // Otherwise return the highest version we do support
                        FFA_VERSION
                    };

                    DirectMsgArgs::VersionResp {
                        version: Some(out_version),
                    }
                } else {
                    panic!("Unexpected source ID ({src_id:#x}) or destination ID ({dst_id:#x})");
                };

                // Return result and wait for the next test index.
                message = direct_response(dst_id, src_id, response_args).unwrap();
            }
            _ => {
                panic!("Unexpected FF-A interface returned: {:?}", message)
            }
        }
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
