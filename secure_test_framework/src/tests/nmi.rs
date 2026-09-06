// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests FEAT_NMI context preservation across world switches.
//!
//! FEAT_NMI introduces the ALLINT bit into PSTATE.
//! This test checks that changing ALLINT in one world does not corrupt the
//! value observed in the other world after a switch through EL3. The value is read and written
//! through either SPSR_EL2 or SPSR_EL1.

use crate::{
    current_el, expect,
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        normal_world_test,
    },
};
use arm_sysregs::{
    el1::{
        accessors::{read_id_aa64pfr1_el1, read_spsr_el1, write_spsr_el1},
        registers::SpsrEl1,
    },
    el2::{
        accessors::{read_spsr_el2, write_spsr_el2},
        registers::SpsrEl2,
    },
};

/// Writes the requested value to either SPSR_EL2's or SPSR_EL1's ALLINT bit in the secure world.
fn write_allint_secure([allint, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        2 => {
            let spsr_before = read_spsr_el2();

            let spsr_new = if allint != 0 {
                spsr_before | SpsrEl2::ALLINT
            } else {
                spsr_before & (!SpsrEl2::ALLINT)
            };

            // Safety: FEAT_NMI is supported, writing the ALLINT bit is allowed.
            unsafe {
                write_spsr_el2(spsr_new);
            }
        }
        1 => {
            let spsr_before = read_spsr_el1();

            let spsr_new = if allint != 0 {
                spsr_before | SpsrEl1::ALLINT
            } else {
                spsr_before & (!SpsrEl1::ALLINT)
            };

            // Safety: FEAT_NMI is supported, writing the ALLINT bit is allowed.
            unsafe {
                write_spsr_el1(spsr_new);
            }
        }
        _ => return Err(()),
    }

    Ok([current_el().into(), 0, 0, 0])
}

normal_world_test!(test_nmi, helper = write_allint_secure);

/// Checks that the value of ALLINT in SPSR_EL1/SPSR_EL2 is preserved across world switches.
fn test_nmi(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64pfr1_el1().is_feat_nmi_present() {
        return Err(TestError::Ignored);
    }
    let allint_el2 = read_spsr_el2().contains(SpsrEl2::ALLINT);
    expect!(!allint_el2);

    let allint_el1 = read_spsr_el1().contains(SpsrEl1::ALLINT);
    expect!(!allint_el1);

    // Enable ALLINT in secure world.
    let [secure_el, _, _, _] = helper([1, 0, 0])?;

    // Verify that ALLINT in normal world is unaffected by secure world changes.
    match secure_el {
        2 => expect!(!read_spsr_el2().contains(SpsrEl2::ALLINT)),
        1 => expect!(!read_spsr_el1().contains(SpsrEl1::ALLINT)),
        _ => return Err(TestError::Failed),
    }

    // Write back 0 to the ALLINT bit to not affect other tests.
    helper([0, 0, 0])?;

    Ok(())
}
