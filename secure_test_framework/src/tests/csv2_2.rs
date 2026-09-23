// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! CSV2_2 tests

use crate::{
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        expect::expect_eq, normal_world_test,
    },
    util::current_el,
};
use arm_sysregs::{
    el1::accessors::read_id_aa64pfr0_el1,
    el2::{
        accessors::{read_scxtnum_el2, write_scxtnum_el2},
        registers::ScxtnumEl2,
    },
};

const TEST_SCXTNUM_NS: ScxtnumEl2 = ScxtnumEl2::empty().with_scxtnum(0x0139_0139_0139_0139);
const TEST_SCXTNUM_S: ScxtnumEl2 = ScxtnumEl2::empty().with_scxtnum(0x6600_6600_6600_6600);

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

normal_world_test!(test_csv2_2, helper = test_csv2_2_helper);

/// Tests FEAT_CSV2_2 availability and EL2 context saving.
///
/// Sets SCXTNUM_EL2 in NS-EL2, then clobbers SCXTNUM_EL2 in S-EL2. Skips the test if the secure helper is
/// run in EL1. Expects the SCXTNUM_EL2 register to be saved across context switches.
pub fn test_csv2_2(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64pfr0_el1().is_feat_csv2_2_present() {
        return Err(TestError::Ignored);
    }

    // SAFETY: The value written to SCXTNUM_EL2 is valid.
    unsafe {
        write_scxtnum_el2(TEST_SCXTNUM_NS);
    }

    match helper(TestHelperRequest::default())
        .and_then(|[test_status, ..]| TestStatus::try_from(test_status))
    {
        Ok(TestStatus::Success) => expect_eq!(read_scxtnum_el2(), TEST_SCXTNUM_NS),
        Ok(TestStatus::Skip) => return Err(TestError::Ignored),
        _ => return Err(TestError::Failed),
    }

    Ok(())
}

/// Clobbers SCXTNUM_EL2 in the secure world, when running in S-EL2.
fn test_csv2_2_helper(_: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        2 => {
            // SAFETY: The value written to SCXTNUM_EL2 is valid.
            unsafe { write_scxtnum_el2(TEST_SCXTNUM_S) }

            Ok([TestStatus::Success as u64, 0, 0, 0])
        }
        1 => Ok([TestStatus::Skip as u64, 0, 0, 0]),
        _ => Err(()),
    }
}
