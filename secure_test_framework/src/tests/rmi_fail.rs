// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Contains only tests for the case where the RME feature is not present, or the RMM fails to boot.
//! Tests that need the STF_RMM should be placed in `secure_test_framework/src/tests/rmi.rs`.

use crate::framework::{TestResult, expect::expect_eq, normal_world_test};
use smccc::smc64;

normal_world_test!(test_no_rmm);
fn test_no_rmm() -> TestResult {
    const REQUESTED_VERSION: u64 = 0x8;

    let mut args = [0; 17];
    args[0] = REQUESTED_VERSION;

    // Arbitrary RMI command, works with any valid RMI function id.
    const RMI_VERSION: u32 = 0xC400_0150;
    let ret = smc64(RMI_VERSION, args);

    // RME is unsupported, or RMM failed to boot
    expect_eq!(ret[0], u64::MAX);

    Ok(())
}
