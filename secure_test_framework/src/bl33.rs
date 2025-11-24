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
mod pagetable;
mod platform;
mod secondary;
mod tests;
mod util;

#[cfg(feature = "pauth")]
use crate::util::enable_pauth;
use crate::{
    exceptions::set_exception_vector,
    ffa::direct_request,
    framework::{
        TestError, normal_world_test_count, normal_world_tests,
        protocol::{Request, Response},
        run_normal_world_test, secure_world_test_count, secure_world_tests,
    },
    platform::{BL33_IDMAP, Platform, PlatformImpl},
    secondary::secondary_entry,
    util::{NORMAL_WORLD_ID, SECURE_WORLD_ID, current_el},
};
use aarch64_rt::{enable_mmu, entry};
use arm_ffa::Interface;
use arm_sysregs::MpidrEl1;
use core::panic::PanicInfo;
use log::{debug, error, info, warn};
use percore::Cores;
use smccc::{Smc, psci};
use spin::mutex::SpinMutex;

/// The version of FF-A which we support.
const FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 2);

/// An unreasonably high FF-A version number.
const HIGH_FFA_VERSION: arm_ffa::Version = arm_ffa::Version(1, 0xffff);

/// An entry point function may be set for each secondary core. When that core starts it will call
/// the function and unset the entry.
static SECONDARY_ENTRIES: [SpinMutex<Option<fn(u64) -> !>>; PlatformImpl::CORE_COUNT] =
    [const { SpinMutex::new(None) }; PlatformImpl::CORE_COUNT];

enable_mmu!(BL33_IDMAP);

entry!(bl33_main, 4);
fn bl33_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    // Enable PAuth with a dummy key.
    #[cfg(feature = "pauth")]
    enable_pauth(0xC0DED00D_C0DED00D_C0DED00D_C0DED00D);

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
    let mut normal_test_counts = TestResultCounts::default();
    for (test_index, test) in normal_world_tests() {
        if test.secure_handler.is_some() {
            // Tell secure world that the test is starting, so it can use the handler.
            match send_request(Request::StartTest { test_index }) {
                Ok(Response::Success { .. }) => {}
                Ok(Response::Panic) => {
                    panic!("Registering test start with secure world caused panic.");
                }
                Ok(response) => {
                    panic!(
                        "Registering test start returned unexpected response {response:?}, this should never happen."
                    );
                }
                Err(()) => continue,
            }
        }
        match run_normal_world_test(test_index, test) {
            Ok(()) => {
                info!("Normal world test {test_index} {} passed", test.name());
                normal_test_counts.passed += 1;
            }
            Err(TestError::Ignored) => {
                info!("Normal world test {test_index} {} ignored", test.name());
                normal_test_counts.ignored += 1;
            }
            Err(TestError::Failed) => {
                warn!("Normal world test {test_index} {} failed", test.name());
                normal_test_counts.failed += 1;
            }
        }
        if test.secure_handler.is_some() {
            // Tell secure world that the test is finished so it can remove the handler.
            match send_request(Request::StopTest) {
                Ok(Response::Success { .. }) => {}
                Ok(Response::Panic) => {
                    panic!("Registering test stop with secure world caused panic.");
                }
                Ok(response) => {
                    panic!(
                        "Registering test start returned unexpected response {response:?}, this should never happen."
                    );
                }
                Err(()) => continue,
            }
        }
    }

    // Run secure world tests.
    let mut secure_test_counts = TestResultCounts::default();
    for (test_index, test) in secure_world_tests() {
        info!("Secure world test {test_index} {} running...", test.name(),);
        match send_request(Request::RunSecureTest { test_index }) {
            Ok(Response::Success { .. }) => {
                info!("Secure world test {test_index} {} passed", test.name());
                secure_test_counts.passed += 1;
            }
            Ok(Response::Ignored) => {
                info!("Secure world test {test_index} {} ignored", test.name());
                secure_test_counts.ignored += 1;
            }
            Ok(Response::Failure) => {
                warn!("Secure world test {test_index} {} failed", test.name());
                secure_test_counts.failed += 1;
            }
            Ok(Response::Panic) => {
                warn!("Secure world test {test_index} {} panicked", test.name());
                secure_test_counts.failed += 1;
                // We can't continue running other tests after one panics.
                break;
            }
            Err(()) => {}
        }
    }

    info!(
        "{}/{} tests passed in normal world, {} ignored, {} failed",
        normal_test_counts.passed,
        normal_world_test_count(),
        normal_test_counts.ignored,
        normal_test_counts.failed,
    );
    info!(
        "{}/{} tests passed in secure world, {} ignored, {} failed",
        secure_test_counts.passed,
        secure_world_test_count(),
        secure_test_counts.ignored,
        secure_test_counts.failed,
    );

    let ret = psci::system_off::<Smc>();
    panic!("PSCI_SYSTEM_OFF returned {:?}", ret);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct TestResultCounts {
    ignored: usize,
    passed: usize,
    failed: usize,
}

extern "C" fn secondary_main(arg: u64) -> ! {
    set_exception_vector();
    gicv3::init_core();

    let core_index = PlatformImpl::core_index();
    debug!("BL33 secondary core {core_index} starting with arg {arg}.");

    let Some(entry) = SECONDARY_ENTRIES[core_index].lock().take() else {
        panic!("Core {core_index} started but no entry point set.");
    };
    entry(arg)
}

/// Calls PSCI CPU_ON to start the secondary CPU with the given PSCI MPIDR value.
pub fn start_secondary(psci_mpidr: u64, entry: fn(u64) -> !, arg: u64) -> Result<(), psci::Error> {
    let core_index = PlatformImpl::core_position(MpidrEl1::from_psci_mpidr(psci_mpidr));
    if SECONDARY_ENTRIES[core_index]
        .lock()
        .replace(entry)
        .is_some()
    {
        error!("Secondary entry point was already set");
    }
    psci::cpu_on::<Smc>(psci_mpidr, secondary_entry as *const () as _, arg)
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
        Response::Ignored => {
            panic!(
                "Secure world test helper {test_index} returned ignored, this should never happen."
            );
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
