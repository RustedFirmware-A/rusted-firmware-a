// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FEAT_HCX architecture extension.

use crate::{
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        expect::expect_eq, normal_world_test,
    },
    util::current_el,
};
use arm_sysregs::{
    HcrxEl2, read_hcrx_el2, read_id_aa64mmfr1_el1, read_id_aa64pfr2_el1, write_hcrx_el2,
};
use log::debug;

/// Used by the secure world helper to signal whether the test should be skipped.
#[repr(u8)]
enum TestStatus {
    Success = 0,
    Skip = 1,
}

impl TryFrom<u64> for TestStatus {
    type Error = ();
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TestStatus::Success),
            1 => Ok(TestStatus::Skip),
            _ => Err(()),
        }
    }
}

normal_world_test!(
    test_hcx_context_switch,
    helper = test_hcx_context_switch_helper
);

/// Inverts HCRX_EL2.ENFPM in the secure world, if running in S-EL2. Leaves other bits untouched.
/// Otherwise returns a value to signal that the test should be skipped.
fn test_hcx_context_switch_helper(_: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    if current_el() != 2 {
        return Ok([TestStatus::Skip as u64, 0, 0, 0]);
    }

    // This will trap to EL3 if SCR_EL3.HXEn is not set in the secure world.
    let hcrx_before = read_hcrx_el2();

    // SAFETY: All bits other than EnFPM stem from a saved value. Both possible values of EnFPM are
    // valid.
    unsafe {
        write_hcrx_el2(hcrx_before ^ HcrxEl2::ENFPM);
    }

    Ok([TestStatus::Success as u64, 0, 0, 0])
}

/// Checks that HCRX_EL2 is context switched correctly. Clobbers fields of HCRX_EL2 which belong to
/// implemented architecture extensions. Currently, only EnFPM (FEAT_FPMR) is supported by RF-A
/// among the fields, therefore when FEAT_FPMR is not supported, the test will be skipped, as all
/// fields would be RES0, leaving context switching to be untestable.
/// In case the secure helper of the test is running in S-EL1, the test will also be skipped,
/// because reading/writing HCRX_EL2 would trap.
fn test_hcx_context_switch(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr1_el1().is_feat_hcx_present() {
        debug!("FEAT_HCX not present, skipping test.");
        return Err(TestError::Ignored);
    }

    if !read_id_aa64pfr2_el1().is_feat_fpmr_present() {
        debug!("No optional features implemented and present for context switch testing.");
        return Err(TestError::Ignored);
    }

    // This will trap to EL3 if SCR_EL3.HXEn is not set in the normal world.
    let expected = read_hcrx_el2();

    // Flip HCRX_EL2.EnFPM in the secure world.
    // If the secure side is running in S-EL1, skip the test.
    if let TestStatus::Skip = helper(TestHelperRequest::default())
        .and_then(|[test_status, ..]| TestStatus::try_from(test_status))?
    {
        debug!("The secure helper is running in S-EL1 and cannot read HCRX_EL2. Skipping test.");
        return Err(TestError::Ignored);
    }

    expect_eq!(read_hcrx_el2(), expected);

    // Flip HCRX_EL2.EnFPM again in the secure world, returning to the original value.
    // It is assumed that the secure side is S-EL2, otherwise the previous check would have skipped
    // the test.
    helper(TestHelperRequest::default())?;

    Ok(())
}

normal_world_test!(test_hcx);

/// Checks that HCRX_EL2 is available from EL2.
fn test_hcx() -> TestResult {
    if !read_id_aa64mmfr1_el1().is_feat_hcx_present() {
        debug!("FEAT_HCX not present, skipping test.");
        return Err(TestError::Ignored);
    }

    // This will trap to EL3 if SCR_EL3.HXEn is not set in the normal world.
    let _ = read_hcrx_el2();

    Ok(())
}
