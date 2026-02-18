// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Realm Management Interface tests, for cases where the STF_RMM successfully boots.
//! Tests that expect a boot failure, or lack the the RME feature should be placed in
//! `secure_test_framework/src/tests/rmi_fail.rs`.

use crate::framework::{TestResult, expect::expect_eq, normal_world_test};
use crate::{
    expect,
    framework::expect::fail,
    platform::{Platform, PlatformImpl},
    rmi::{RMI_GRANULE_DELEGATE, RMI_GRANULE_UNDELEGATE, RMI_VERSION, RmiStatusCode},
};
use smccc::smc64;

macro_rules! check_rmi_status {
    ($status:expr, $expected:expr) => {
        let status_low = RmiStatusCode::try_from(($status & 0xFF) as u8);

        match status_low {
            Ok(status) => {
                if status != $expected {
                    fail!(
                        "RMI command returned {:?}, expected {:?}",
                        status,
                        $expected
                    );
                }
            }
            Err(_) => {
                fail!(
                    "RMI return code does not match RmiStatusCode: 0x{:x}",
                    $status
                );
            }
        };
    };
}

normal_world_test!(test_rmm_version);
fn test_rmm_version() -> TestResult {
    const REQUESTED_VERSION: u64 = 0x8;

    let mut args = [0; 17];
    args[0] = REQUESTED_VERSION;

    let ret = smc64(RMI_VERSION, args);

    if ret[0] == u64::MAX {
        fail!("RMI_VERSION returned 0x{:x}", ret[0]);
    }

    let lower = ret[1];
    let higher = ret[2];

    expect_eq!(lower >> 32, 0);
    expect_eq!(higher >> 32, 0);
    expect!(lower <= higher);
    expect!(ret[3..].iter().all(|r| *r == 0));

    match ret[0] {
        0 => {
            expect_eq!(lower, REQUESTED_VERSION);
        }
        1 => expect!(lower != REQUESTED_VERSION),
        v => fail!("Invalid return code from RMI_VERSION: {v}"),
    }

    Ok(())
}

normal_world_test!(test_granule_delegate);
fn test_granule_delegate() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = PlatformImpl::PAS_CONFIG
        .non_secure_start
        .try_into()
        .unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::Success);

    // Undo changes and test undelegation
    let ret = smc64(RMI_GRANULE_UNDELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::Success);

    Ok(())
}

normal_world_test!(test_granule_delegate_badpas_any);
fn test_granule_delegate_badpas_any() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = PlatformImpl::PAS_CONFIG.any_start.try_into().unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}

normal_world_test!(test_granule_delegate_badpas_realm);
fn test_granule_delegate_badpas_realm() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = PlatformImpl::PAS_CONFIG.realm_start.try_into().unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}

normal_world_test!(test_granule_delegate_badpas_secure);
fn test_granule_delegate_badpas_secure() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = PlatformImpl::PAS_CONFIG.secure_start.try_into().unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}

normal_world_test!(test_granule_delegate_badpas_root);
fn test_granule_delegate_badpas_root() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = PlatformImpl::PAS_CONFIG.root_start.try_into().unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}

normal_world_test!(test_granule_delegate_unaligned);
fn test_granule_delegate_unaligned() -> TestResult {
    let mut args = [0; 17];
    let addr: u64 = (PlatformImpl::PAS_CONFIG.non_secure_start + 1)
        .try_into()
        .unwrap();
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}

normal_world_test!(test_granule_delegate_oom);
fn test_granule_delegate_oom() -> TestResult {
    let mut args = [0; 17];
    // Based on current GPT config, this is the first address outside of the range covered by the GPT.
    let addr: u64 = 0x100_0000_0000;
    args[0] = addr;

    let ret = smc64(RMI_GRANULE_DELEGATE, args);
    check_rmi_status!(ret[0], RmiStatusCode::ErrorInput);

    Ok(())
}
