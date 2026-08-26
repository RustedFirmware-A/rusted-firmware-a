// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests FEAT_PAN context preservation across world switches.
//!
//! FEAT_PAN introduces the PAN bit into PSTATE, and the PAN register aliasing access to it.

use crate::framework::{
    TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
    expect::expect_eq, normal_world_test,
};
use arm_sysregs::{
    el0::{
        accessors::{read_pan, write_pan},
        registers::Pan,
    },
    el1::accessors::read_id_aa64mmfr1_el1,
};

/// Sets the PAN bit in PSTATE in the secure world.
fn test_pan_helper([value, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    // Safety: FEAT_PAN is supported.
    unsafe {
        write_pan(Pan::from_bits_retain(value));
    }

    Ok(TestHelperResponse::default())
}

normal_world_test!(test_pan, helper = test_pan_helper);

/// Uses the helper to set PAN in the secure world, and checks that the normal world PAN value
/// isn't modified.
/// The secure world PAN is expected to be zero at the start.
fn test_pan(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr1_el1().is_feat_pan_present() {
        return Err(TestError::Ignored);
    }

    expect_eq!(read_pan(), Pan::empty());

    let _ = helper([Pan::PAN.bits(), 0, 0])?;

    // Check that setting PAN in the secure world does not affect the normal world.
    expect_eq!(read_pan(), Pan::empty());

    // Set the secure world PAN to it's original value.
    let _ = helper([Pan::empty().bits(), 0, 0])?;

    Ok(())
}
