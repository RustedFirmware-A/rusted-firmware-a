// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use smccc::smc64;

use crate::{
    expect,
    framework::{TestResult, expect::expect_eq, normal_world_test},
};

normal_world_test!(test_rmm_version);
fn test_rmm_version() -> TestResult {
    const REQUESTED_VERSION: u64 = 0x8;

    let mut args = [0; 17];
    args[0] = REQUESTED_VERSION;

    let ret = smc64(0xC400_0150u32, args);

    // Call not supported, i.e. there is no RMMD.
    if ret[0] == u64::MAX {
        return Ok(());
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
        _ => expect!(false),
    }

    Ok(())
}
