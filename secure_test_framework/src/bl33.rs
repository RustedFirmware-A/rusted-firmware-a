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
mod sysregs;
mod tests;
mod util;

use crate::{
    exceptions::set_exception_vector,
    ffa::direct_request,
    framework::{
        normal_world_test_count, normal_world_tests,
        protocol::{Request, Response},
        run_normal_world_test, secure_world_test_count, secure_world_tests,
    },
    platform::{Platform, PlatformImpl},
    util::{NORMAL_WORLD_ID, SECURE_WORLD_ID, current_el},
};
use aarch64_rt::entry;
use arm_ffa::Interface;
use core::panic::PanicInfo;
use log::{error, info, warn};
use smccc::{Smc, psci};

/// The version of FF-A which we support.
const FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 2);

/// An unreasonably high FF-A version number.
const HIGH_FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0xffff);

entry!(bl33_main, 4);
fn bl33_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink).unwrap();

    set_exception_vector();
    gicv3::init(false);

    info!(
        "Rust BL33 starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        x0,
        x1,
        x2,
        x3,
    );

    heap::init();

    // Test what happens if we try a much higher version.
    let spmc_supported_ffa_version = ffa::version(HIGH_FFA_VERSION).expect("FFA_VERSION failed");
    info!("SPMC supports FF-A version {}", spmc_supported_ffa_version);
    assert!(spmc_supported_ffa_version >= FFA_VERSION);
    assert!(spmc_supported_ffa_version < HIGH_FFA_VERSION);
    // Negotiate the FF-A version we actually support. This must happen before any other FF-A calls.
    assert_eq!(ffa::version(FFA_VERSION), Ok(FFA_VERSION));

    // Run normal world tests.
    let mut passing_normal_test_count = 0;
    for (test_index, test) in normal_world_tests() {
        if test.secure_handler.is_some() {
            // Tell secure world that the test is starting, so it can use the handler.
            match send_request(Request::StartTest { test_index }) {
                Ok(Response::Success { .. }) => {}
                Ok(Response::Failure) => {
                    warn!("Registering test start with secure world failed.");
                    continue;
                }
                Ok(Response::Panic) => {
                    panic!("Registering test start with secure world caused panic.");
                }
                Err(()) => continue,
            }
        }
        if run_normal_world_test(test_index, test).is_ok() {
            info!("Normal world test {} passed", test.name());
            passing_normal_test_count += 1;
        } else {
            warn!("Normal world test {} failed", test.name());
        }
        if test.secure_handler.is_some() {
            // Tell secure world that the test is finished so it can remove the handler.
            match send_request(Request::StopTest) {
                Ok(Response::Success { .. }) => {}
                Ok(Response::Failure) => {
                    warn!("Registering test stop with secure world failed.");
                    continue;
                }
                Ok(Response::Panic) => {
                    panic!("Registering test stop with secure world caused panic.");
                }
                Err(()) => continue,
            }
        }
    }
    info!(
        "{}/{} tests passed in normal world",
        passing_normal_test_count,
        normal_world_test_count(),
    );

    // Run secure world tests.
    let mut passing_secure_test_count = 0;
    for (test_index, test) in secure_world_tests() {
        info!(
            "Requesting secure world test {} run: {}",
            test_index,
            test.name(),
        );
        match send_request(Request::RunSecureTest { test_index }) {
            Ok(Response::Success { .. }) => {
                info!("Secure world test {} passed", test_index);
                passing_secure_test_count += 1;
            }
            Ok(Response::Failure) => {
                warn!("Secure world test {} failed", test_index);
            }
            Ok(Response::Panic) => {
                warn!("Secure world test {} panicked", test_index);
                // We can't continue running other tests after one panics.
                break;
            }
            Err(()) => {}
        }
    }
    info!(
        "{}/{} tests passed in secure world",
        passing_secure_test_count,
        secure_world_test_count(),
    );

    let ret = psci::system_off::<Smc>();
    panic!("PSCI_SYSTEM_OFF returned {:?}", ret);
}

/// Sends a direct request to the secure world and returns the response.
///
/// Panics if there is an error parsing the FF-A response or the endpoint IDs do not match what we
/// expect. Returns an error if the response is not an FF-A direct message response or it can't be
/// parsed as an STF response.
fn send_request(request: Request) -> Result<Response, ()> {
    let result = direct_request(NORMAL_WORLD_ID, SECURE_WORLD_ID, request.into())
        .expect("Failed to parse direct request response");
    let Interface::MsgSendDirectResp {
        src_id,
        dst_id,
        args,
    } = result
    else {
        warn!("Unexpected response {:?}", result);
        return Err(());
    };
    assert_eq!(src_id, SECURE_WORLD_ID);
    assert_eq!(dst_id, NORMAL_WORLD_ID);

    Response::try_from(args).map_err(|e| {
        warn!("{}", e);
    })
}

/// Sends a direct request to the secure world to run the secure helper component for the given test
/// index.
fn call_test_helper(test_index: usize, args: [u64; 3]) -> Result<[u64; 4], ()> {
    match send_request(Request::RunTestHelper { test_index, args })? {
        Response::Success { return_value } => Ok(return_value),
        Response::Failure => {
            warn!("Secure world test helper {} failed", test_index);
            Err(())
        }
        Response::Panic => {
            // We can't continue running other tests after the secure world panics, so we panic
            // too.
            panic!("Secure world test helper {} panicked", test_index);
        }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    let _ = psci::system_off::<Smc>();
    loop {}
}
