// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FEAT_PAN3 architecture extension.
//!
//! FEAT_PAN3 introduces the EPAN bit to SCTLR_EL1 and SCTLR_EL2.

use crate::{
    current_el, expect,
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        normal_world_test,
    },
};
use arm_sysregs::{
    el1::accessors::read_id_aa64mmfr1_el1,
    el1::{
        accessors::{read_sctlr_el1, write_sctlr_el1},
        registers::SctlrEl1,
    },
    el2::{
        accessors::{read_sctlr_el2, write_sctlr_el2},
        registers::SctlrEl2,
    },
};

/// Writes the EPAN bit of SctlrEl2 in the secure world.
fn write_epan_secure([epan, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        2 => {
            let mut reg = read_sctlr_el2();

            reg.set(SctlrEl2::EPAN, epan != 0);

            // Safety: FEAT_PAN3 is supported, writing the EPAN bit is allowed.
            unsafe {
                write_sctlr_el2(reg);
            }
        }
        1 => {
            let mut reg = read_sctlr_el1();

            reg.set(SctlrEl1::EPAN, epan != 0);

            // Safety: FEAT_PAN3 is supported, writing the EPAN bit is allowed.
            unsafe {
                write_sctlr_el1(reg);
            }
        }
        _ => return Err(()),
    }

    Ok([current_el().into(), 0, 0, 0])
}

normal_world_test!(test_sctlr_epan, helper = write_epan_secure);
fn test_sctlr_epan(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr1_el1().is_feat_pan3_present() {
        return Err(TestError::Ignored);
    }

    // Assert EPAN is off by default.
    expect!(!read_sctlr_el1().contains(SctlrEl1::EPAN));
    expect!(!read_sctlr_el2().contains(SctlrEl2::EPAN));

    // Enable EPAN in secure world. The secure world EPAN is assumed to be zero at the start.
    let [secure_el, ..] = helper([1, 0, 0])?;

    // Verify that EPAN in normal world is unaffected by secure world changes.
    match secure_el {
        2 => expect!(!read_sctlr_el2().contains(SctlrEl2::EPAN)),
        1 => expect!(!read_sctlr_el1().contains(SctlrEl1::EPAN)),
        _ => return Err(TestError::Failed),
    }

    // Set EPAN to its original value in secure world.
    let _ = helper([0, 0, 0])?;

    Ok(())
}
