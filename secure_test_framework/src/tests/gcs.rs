// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! GCS tests
//! Tests whether GCS-specific context switched registers are properly
//! saved/restored between context switches.

use crate::{
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        expect::expect_eq, normal_world_test,
    },
    util::current_el,
};
use arm_sysregs::{
    el1::accessors::read_id_aa64pfr1_el1,
    el2::{
        accessors::{read_gcscr_el2, read_gcspr_el2, write_gcscr_el2, write_gcspr_el2},
        registers::{GcscrEl2, GcsprEl2},
    },
};

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

normal_world_test!(test_gcs, helper = test_gcs_helper);

// Arbitrary values used for testing context saving/restoration
const GCSCR_FLAGS_ORIGINAL: [GcscrEl2; 1] = [GcscrEl2::STREN];
const GCSPR_PTR_ORIGINAL: u64 = 0xBEEF_F00D_0000_0000;
const GCSCR_FLAGS_MODIFIED: [GcscrEl2; 2] = [GcscrEl2::STREN, GcscrEl2::PUSHMEN];
const GCSPR_PTR_MODIFIED: u64 = 0xDEAD_BEEF_0000_0000;

fn as_reg_values(gcscr_flags: &[GcscrEl2], gcspr_ptr: u64) -> (GcscrEl2, GcsprEl2) {
    let gcscr_el2 = gcscr_flags
        .iter()
        .fold(GcscrEl2::empty(), |acc, next| acc | *next);
    // PTR needs to be shifted, because the PTR field does not store the LSBs of the pointer.
    // This effectively truncates the pointer, which is done by the right shift here, and
    // the left shift in the setter function
    let gcspr_el2 =
        GcsprEl2::with_ptr_63_3(GcsprEl2::empty(), gcspr_ptr >> GcsprEl2::PTR_63_3_SHIFT);

    (gcscr_el2, gcspr_el2)
}

/// Tests FEAT_GCS availability and EL2 context saving.
///
/// Sets GCSCR and GCSPR in NS-EL2, then modifies them in S-EL2. Skips the test if the secure helper is
/// run in EL1. Expects the registers to be saved across context switches.
pub fn test_gcs(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64pfr1_el1().is_feat_gcs_present() {
        return Err(TestError::Ignored);
    }

    let (gcscr_original, gcspr_original) = as_reg_values(&GCSCR_FLAGS_ORIGINAL, GCSPR_PTR_ORIGINAL);
    // SAFETY: the values written to GCSCR_EL2 and GCSPR_EL2 are valid
    unsafe {
        write_gcscr_el2(gcscr_original);
        write_gcspr_el2(gcspr_original);
    }

    // Sanity check: registers were set correctly
    expect_eq!(read_gcscr_el2(), gcscr_original);
    expect_eq!(read_gcspr_el2(), gcspr_original);

    match helper(TestHelperRequest::default())
        .and_then(|[test_status, ..]| TestStatus::try_from(test_status))
    {
        Ok(TestStatus::Success) => {
            expect_eq!(read_gcscr_el2(), gcscr_original);
            expect_eq!(read_gcspr_el2(), gcspr_original);
        }
        Ok(TestStatus::Skip) => return Err(TestError::Ignored),
        _ => return Err(TestError::Failed),
    }

    Ok(())
}

/// Modifies GCSCR_EL2 and GCSPR_EL2 in the secure world when running in SEL2.
fn test_gcs_helper(_: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        2 => {
            let (gcscr_modified, gcspr_modified) =
                as_reg_values(&GCSCR_FLAGS_MODIFIED, GCSPR_PTR_MODIFIED);
            // SAFETY: The values written to GCSCR_EL2 and GCSPR_EL2 are valid
            unsafe {
                write_gcscr_el2(gcscr_modified);
                write_gcspr_el2(gcspr_modified);
            }

            Ok([TestStatus::Success as u64, 0, 0, 0])
        }
        1 => Ok([TestStatus::Skip as u64, 0, 0, 0]),
        _ => Err(()),
    }
}
