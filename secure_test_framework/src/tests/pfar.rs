// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! PFAR tests

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
        accessors::{read_pfar_el2, write_pfar_el2},
        registers::PfarEl2,
    },
};

const TEST_PA48_NS: u64 = 0xfedc_ba98_7654;
const TEST_PA48_S: u64 = 0xcba9_8765_4324;

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

normal_world_test!(test_pfar, helper = test_pfar_helper);

/// Tests FEAT_PFAR availability and EL2 context saving.
///
/// Sets PFAR_EL2 in NS-EL2, then clobbers PFAR_EL2 in S-EL2. Skips the test if the secure helper is
/// run in EL1. Expects the PFAR_EL2 register to be saved across context switches.
pub fn test_pfar(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64pfr1_el1().is_feat_pfar_present() {
        return Err(TestError::Ignored);
    }

    // SAFETY: The value written to PFAR_EL2 is valid.
    unsafe {
        // The NS bit is used to indicate that PFAR_EL2 holds a non-secure physical address, to
        // mimic a real scenario. Practically, this does not make a difference in how the register
        // is handled.
        write_pfar_el2(PfarEl2::NS.with_pa(TEST_PA48_NS));
    }

    // `expected_pfar_el2` is not guaranteed to be the same as the value written, because it is
    // masked to be as many bits as a physical address.
    // From the Architecture Reference Manual:
    // > For implementations with fewer than 48 physical address bits, the corresponding upper bits
    // > in this field are RES0.
    let expected_pfar_el2 = read_pfar_el2();

    match helper(TestHelperRequest::default())
        .and_then(|[test_status, ..]| TestStatus::try_from(test_status))
    {
        Ok(TestStatus::Success) => expect_eq!(read_pfar_el2(), expected_pfar_el2),
        Ok(TestStatus::Skip) => return Err(TestError::Ignored),
        _ => return Err(TestError::Failed),
    }

    Ok(())
}

/// Clobbers PFAR_EL2 in the secure world, when running in SEL2.
fn test_pfar_helper(_: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        2 => {
            // SAFETY: The value written to PFAR_EL2 is valid.
            unsafe { write_pfar_el2(PfarEl2::empty().with_pa(TEST_PA48_S)) }

            Ok([TestStatus::Success as u64, 0, 0, 0])
        }
        1 => Ok([TestStatus::Skip as u64, 0, 0, 0]),
        _ => Err(()),
    }
}
