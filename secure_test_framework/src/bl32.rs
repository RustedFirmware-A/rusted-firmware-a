// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

mod exceptions;
mod ffa;
mod framework;
mod gicv3;
mod logger;
mod platform;
mod tests;
mod util;

use crate::{
    exceptions::set_exception_vector,
    ffa::{call, direct_response, msg_wait},
    framework::{
        protocol::{ParseRequestError, Request, Response},
        run_secure_world_test, run_test_ffa_handler, run_test_helper,
    },
    gicv3::handle_group1_interrupt,
    platform::{Platform, PlatformImpl},
    util::{NORMAL_WORLD_ID, SECURE_WORLD_ID, SPMC_DEFAULT_ID, SPMD_DEFAULT_ID, current_el},
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, FfaError, Interface, SuccessArgsIdGet, Version};
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

    let mut current_test_index = None;

    // Wait for the first test index.
    let mut message = msg_wait(None).unwrap();

    loop {
        let response = match message {
            Interface::Interrupt { .. } => {
                handle_group1_interrupt();
                Interface::NormalWorldResume
            }
            Interface::MsgSendDirectReq {
                src_id,
                dst_id,
                args,
            } => {
                let response_args = handle_direct_message(
                    src_id,
                    dst_id,
                    args,
                    spmc_id,
                    spmd_id,
                    &mut current_test_index,
                );

                Interface::MsgSendDirectResp {
                    src_id: dst_id,
                    dst_id: src_id,
                    args: response_args,
                }
            }
            _ => {
                if let Some(current_test_index) = current_test_index {
                    if let Some(response) = run_test_ffa_handler(current_test_index, message) {
                        response
                    } else {
                        panic!("Test couldn't handle FF-A interface {:?}", message);
                    }
                } else {
                    panic!("Unexpected FF-A interface returned: {:?}", message)
                }
            }
        };
        // Return result and wait for the next test index.
        message = call(response).unwrap()
    }
}

/// Handles a direct message request and returns a response to send back.
fn handle_direct_message(
    src_id: u16,
    dst_id: u16,
    args: DirectMsgArgs,
    spmc_id: u16,
    spmd_id: u16,
    current_test_index: &mut Option<usize>,
) -> DirectMsgArgs {
    if src_id == NORMAL_WORLD_ID && dst_id == SECURE_WORLD_ID {
        match Request::try_from(args) {
            Ok(request) => handle_request(request, current_test_index).into(),
            Err(ParseRequestError::InvalidDirectMsgType(args)) => {
                panic!(
                    "Received unexpected direct message type from Normal World: {:?}",
                    args
                );
            }
            Err(e @ ParseRequestError::InvalidRequestCode(_)) => {
                error!("{}", e);
                Response::Failure.into()
            }
        }
    } else if src_id == spmd_id && dst_id == spmc_id {
        let DirectMsgArgs::VersionReq { version } = args else {
            panic!("Received unexpected direct message type from SPMD.");
        };
        handle_version_request(version)
    } else {
        panic!("Unexpected source ID ({src_id:#x}) or destination ID ({dst_id:#x})");
    }
}

fn handle_version_request(version: Version) -> DirectMsgArgs {
    let out_version = if version.is_compatible_to(&FFA_VERSION) {
        // If NWd queries a version that we're compatible with, return the same
        let nwd_supported_ffa_version = version;
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
}

/// Handles a request from the normal world BL33.
fn handle_request(request: Request, current_test_index: &mut Option<usize>) -> Response {
    match request {
        Request::RunSecureTest { test_index } => {
            if run_secure_world_test(test_index).is_ok() {
                Response::Success {
                    return_value: [0; 4],
                }
            } else {
                Response::Failure
            }
        }
        Request::RunTestHelper { test_index, args } => match run_test_helper(test_index, args) {
            Ok(return_value) => Response::Success { return_value },
            Err(()) => Response::Failure,
        },
        Request::StartTest { test_index } => {
            assert_eq!(*current_test_index, None);
            *current_test_index = Some(test_index);
            Response::Success {
                return_value: [0; 4],
            }
        }
        Request::StopTest => {
            assert!(current_test_index.is_some());
            *current_test_index = None;
            Response::Success {
                return_value: [0; 4],
            }
        }
    }
}

fn call_test_helper(_index: usize, _args: [u64; 3]) -> Result<[u64; 4], ()> {
    panic!("call_test_helper shouldn't be called from secure world tests");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Log the panic message
    error!("{}", info);
    // Tell normal world that the test panicked. In case we get another request anyway, keep
    // sending the same response.
    loop {
        let _ = direct_response(SECURE_WORLD_ID, NORMAL_WORLD_ID, Response::Panic.into());
    }
}
