// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use smccc::smc64;

use crate::framework::{TestResult, expect::expect_eq, normal_world_test};

#[cfg(all(feature = "rme", not(feature = "test_rmm_fail")))]
use crate::{expect, framework::expect::fail};

const RMM_RMI_REQ_VERSION: u32 = 0xC400_0150;

#[cfg(any(not(feature = "rme"), feature = "test_rmm_fail"))]
normal_world_test!(test_no_rmm);
#[cfg(any(not(feature = "rme"), feature = "test_rmm_fail"))]
fn test_no_rmm() -> TestResult {
    const REQUESTED_VERSION: u64 = 0x8;

    let mut args = [0; 17];
    args[0] = REQUESTED_VERSION;

    let ret = smc64(RMM_RMI_REQ_VERSION, args);

    // RME is unsupported, or RMM failed to boot
    expect_eq!(ret[0], u64::MAX);

    Ok(())
}

#[cfg(all(feature = "rme", not(feature = "test_rmm_fail")))]
normal_world_test!(test_rmm_version);
#[cfg(all(feature = "rme", not(feature = "test_rmm_fail")))]
fn test_rmm_version() -> TestResult {
    const REQUESTED_VERSION: u64 = 0x8;

    let mut args = [0; 17];
    args[0] = REQUESTED_VERSION;

    let ret = smc64(RMM_RMI_REQ_VERSION, args);

    if ret[0] == u64::MAX {
        fail!("RMM_RMI_REQ_VERSION returned 0x{:x}", ret[0]);
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
        v => fail!("Invalid return code from RMM_RMI_REQ_VERSION: {v}"),
    }

    Ok(())
}
