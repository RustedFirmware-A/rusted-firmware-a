// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests FEAT_DIT context preservation across world switches.
//!
//! DIT is a part of PSTATE. This test checks that changing DIT in one world does not corrupt the
//! value observed in the other world after a switch through EL3.

use crate::framework::{
    TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult, expect::expect_eq,
    normal_world_test,
};
use arm_sysregs::{Dit, read_dit, write_dit};

/// Updates the secure-world DIT, and returns its value before the write.
fn test_dit_helper([dit_value, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    let dit_before = read_dit();

    write_dit(Dit::from_bits_retain(dit_value));

    Ok([dit_before.bits(), 0, 0, 0])
}

normal_world_test!(test_dit, helper = test_dit_helper);

/// Checks that normal-world and secure-world DIT values are preserved across world switches.
/// Assumes DIT to be unset in both the secure and non-secure world when running the test.
fn test_dit(helper: &TestHelperProxy) -> TestResult {
    expect_eq!(read_dit(), Dit::empty());

    // Change DIT in secure world. Assume that it was initially toggled off.
    let [sw_dit, ..] = helper([Dit::DIT.bits(), 0, 0])?;
    expect_eq!(Dit::from_bits_retain(sw_dit), Dit::empty());

    // Verify that DIT in normal world is unaffected by secure world changes.
    expect_eq!(read_dit(), Dit::empty());

    // This part is needed to ensure that PSTATE.DIT is reset to its original state, in order to
    // avoid affecting other tests.
    helper([Dit::empty().bits(), 0, 0])?;

    Ok(())
}
