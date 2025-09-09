// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake SPMC component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

extern crate alloc;

mod exceptions;
mod ffa;
mod framework;
mod gicv3;
mod heap;
mod logger;
mod platform;
mod secondary;
mod sysregs;
mod tests;
mod util;

use crate::{
    exceptions::set_exception_vector,
    ffa::{call, direct_response, msg_wait, secondary_ep_register},
    framework::{
        protocol::{ParseRequestError, Request, Response},
        run_secure_world_test, run_test_ffa_handler, run_test_helper,
    },
    platform::{Platform, PlatformImpl},
    secondary::secondary_entry,
    util::{
        NORMAL_WORLD_ID, SECURE_WORLD_ID, SPMC_DEFAULT_ID, SPMD_DEFAULT_ID, current_el,
        expect_ffa_success,
    },
};
use aarch64_rt::entry;
use arm_ffa::{DirectMsgArgs, FfaError, Interface, SuccessArgsIdGet, Version, WarmBootType};
use arm_psci::ReturnCode;
use core::{
    panic::PanicInfo,
    sync::atomic::{AtomicIsize, Ordering},
};
use log::{error, info, warn};
use percore::Cores;

/// The version of FF-A which we support.
const FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 2);

/// An unreasonably high FF-A version number.
const HIGH_FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0xffff);

/// The index of the currently-running test, or -1 if no test is active.
static CURRENT_TEST_INDEX: AtomicIsize = AtomicIsize::new(-1);

/// Returns the index of the currently-running test, or `None` if no test is active.
fn current_test_index() -> Option<usize> {
    CURRENT_TEST_INDEX.load(Ordering::Acquire).try_into().ok()
}

entry!(bl32_main, 4);
fn bl32_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink).unwrap();

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

    heap::init();

    // Test what happens if we try a much higher version.
    let el3_supported_ffa_version = ffa::version(HIGH_FFA_VERSION).expect("FFA_VERSION failed");
    info!("EL3 supports FF-A version {}", el3_supported_ffa_version);
    assert!(el3_supported_ffa_version >= FFA_VERSION);
    assert!(el3_supported_ffa_version < HIGH_FFA_VERSION);
    // Negotiate the FF-A version we actually support. This must happen before any other FF-A calls.
    assert_eq!(ffa::version(FFA_VERSION), Ok(FFA_VERSION));

    // Register secondary core entry point.
    expect_ffa_success(
        // SAFETY: secondary_entry is a valid secondary entry point that will set up the stack for
        // Rust code to run.
        unsafe { secondary_ep_register(secondary_entry as u64) }
            .expect("FFA_SECONDARY_EP_REGISTER failed"),
    )
    .unwrap();

    message_loop();
}

extern "C" fn secondary_main() -> ! {
    set_exception_vector();

    info!("BL32 secondary core starting");

    message_loop();
}

/// Loops forever handling FF-A direct messages or other FF-A interfaces, to run tests and test
/// helpers.
fn message_loop() -> ! {
    let core_index = PlatformImpl::core_index();
    let spmc_id = get_spmc_id();
    let spmd_id = get_spmd_id();

    // Wait for the first message.
    let mut message = msg_wait(None).unwrap();

    loop {
        let response = match message {
            Interface::MsgSendDirectReq {
                src_id,
                dst_id,
                args,
            } => {
                let response_args = handle_direct_message(src_id, dst_id, args, spmc_id, spmd_id);

                Interface::MsgSendDirectResp {
                    src_id: dst_id,
                    dst_id: src_id,
                    args: response_args,
                }
            }
            _ => {
                if let Some(current_test_index) = current_test_index() {
                    if let Some(response) = run_test_ffa_handler(current_test_index, message) {
                        response
                    } else {
                        panic!("Test couldn't handle FF-A interface {message:?}");
                    }
                } else {
                    panic!("BL32 got unexpected FF-A interface on core {core_index}: {message:?}")
                }
            }
        };
        // Return result and wait for the next test index.
        message = call(response).unwrap()
    }
}

/// Calls `FFA_ID_GET` to get the SPMC ID (i.e. our ID).
///
/// Returns `SPMC_DEFAULT_ID` if the `FFA_ID_GET` call returns `NOT_SUPPORTED`, or panics on any
/// other error.
fn get_spmc_id() -> u16 {
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
}

/// Calls `FFA_SPM_ID_GET` to get the SPMD ID.
///
/// Returns `SPMD_DEFAULT_ID` if the `FFA_SPM_ID_GET` call returns `NOT_SUPPORTED`, or panics on any
/// other error.
fn get_spmd_id() -> u16 {
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
}

/// Handles a direct message request and returns a response to send back.
fn handle_direct_message(
    src_id: u16,
    dst_id: u16,
    args: DirectMsgArgs,
    spmc_id: u16,
    spmd_id: u16,
) -> DirectMsgArgs {
    let core_index = PlatformImpl::core_index();
    if src_id == NORMAL_WORLD_ID && dst_id == SECURE_WORLD_ID {
        match Request::try_from(args) {
            Ok(request) => handle_request(request).into(),
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
        match args {
            DirectMsgArgs::VersionReq { version } if core_index == 0 => {
                handle_version_request(version)
            }
            DirectMsgArgs::PowerPsciReq32 { params } => handle_psci_request32(params),
            DirectMsgArgs::PowerPsciReq64 { params } => handle_psci_request64(params),
            DirectMsgArgs::PowerWarmBootReq { boot_type } => handle_warm_boot_request(boot_type),
            _ => panic!("Received unexpected direct message type from SPMD on core {core_index}."),
        }
    } else {
        panic!(
            "Unexpected source ID ({src_id:#x}) or destination ID ({dst_id:#x}) on core {core_index}."
        );
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

fn handle_psci_request32(_params: [u32; 4]) -> DirectMsgArgs {
    DirectMsgArgs::PowerPsciResp {
        psci_status: (ReturnCode::Success).into(),
    }
}

fn handle_psci_request64(_params: [u64; 4]) -> DirectMsgArgs {
    DirectMsgArgs::PowerPsciResp {
        psci_status: (ReturnCode::Success).into(),
    }
}

fn handle_warm_boot_request(_boot_type: WarmBootType) -> DirectMsgArgs {
    DirectMsgArgs::PowerPsciResp {
        psci_status: (ReturnCode::Success).into(),
    }
}

/// Handles a request from the normal world BL33.
fn handle_request(request: Request) -> Response {
    let core_index = PlatformImpl::core_index();
    let is_primary_core = core_index == 0;
    match request {
        Request::RunSecureTest { test_index } if is_primary_core => {
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
        Request::StartTest { test_index } if is_primary_core => {
            let previous_test_index =
                CURRENT_TEST_INDEX.swap(test_index.try_into().unwrap(), Ordering::AcqRel);
            assert_eq!(previous_test_index, -1);
            Response::Success {
                return_value: [0; 4],
            }
        }
        Request::StopTest if is_primary_core => {
            let previous_test_index = CURRENT_TEST_INDEX.swap(-1, Ordering::AcqRel);
            assert!(!previous_test_index.is_negative());
            Response::Success {
                return_value: [0; 4],
            }
        }
        _ => {
            panic!("Unexpected STF request on secondary core {core_index}: {request:?}");
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
