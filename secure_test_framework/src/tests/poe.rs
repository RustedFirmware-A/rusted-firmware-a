// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FEAT_S1POE and FEAT_S2POE architecture extensions.

use crate::{
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        expect::expect_eq, normal_world_test,
    },
    util::current_el,
};
use arm_sysregs::{
    el1::{
        accessors::{
            read_id_aa64mmfr3_el1, read_por_el1, read_s2por_el1, write_por_el1, write_s2por_el1,
        },
        registers::{PorEl1, S2porEl1},
    },
    el2::{
        accessors::{read_por_el2, write_por_el2},
        registers::PorEl2,
    },
};

normal_world_test!(test_s1poe, helper = test_s1poe_helper);

/// Sets the POR_ELx register in the secure world. It returns the current EL of the secure world
/// component, so the normal world test can check whether the corresponding register has been
/// preserved.
fn test_s1poe_helper([por, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        1 => {
            // Safety: FEAT_S1POE is supported
            unsafe {
                write_por_el1(PorEl1::from_bits_retain(por));
            }
        }
        2 => {
            // Safety: FEAT_S1POE is supported
            unsafe {
                write_por_el2(PorEl2::from_bits_retain(por));
            }
        }
        _ => return Err(()),
    }

    Ok([current_el().into(), 0, 0, 0])
}

/// Uses the helper to set S1POE related system registers in the secure world, and checks that the
/// normal world context has been preserved. The registers are expected to be set to zero during
/// boot.
fn test_s1poe(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr3_el1().is_feat_s1poe_present() {
        return Err(TestError::Ignored);
    }

    expect_eq!(read_por_el1(), PorEl1::empty());
    expect_eq!(read_por_el2(), PorEl2::empty());

    let [secure_el, ..] = helper([1, 0, 0])?;

    // Verify that S1POE registers in normal world are not affected by secure world changes.
    match secure_el {
        1 => {
            expect_eq!(read_por_el1(), PorEl1::empty());
        }
        2 => {
            expect_eq!(read_por_el2(), PorEl2::empty());
        }
        _ => return Err(TestError::Failed),
    }

    // Restore the original secure world state.
    let _ = helper([0, 0, 0])?;

    Ok(())
}

normal_world_test!(test_s2poe, helper = test_s2poe_helper);

/// Sets the S2POR_EL1 register in the secure world if running in EL1. It returns the current EL of
/// the secure world component.
fn test_s2poe_helper([s2por, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    if current_el() == 1 {
        // Safety: FEAT_S2POE is supported
        unsafe {
            write_s2por_el1(S2porEl1::from_bits_retain(s2por));
        }
    }

    Ok([current_el().into(), 0, 0, 0])
}

/// Uses the helper to set S2POE related system registers in the secure world, and checks that the
/// normal world context has been preserved if the secure world helper is deployed to EL1. The
/// registers are expected to be set to zero during boot.
fn test_s2poe(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr3_el1().is_feat_s2poe_present() {
        return Err(TestError::Ignored);
    }

    expect_eq!(read_s2por_el1(), S2porEl1::empty());

    let [secure_el, ..] = helper([1, 0, 0])?;

    if secure_el == 1 {
        // Verify that S2POE registers in normal world are not affected by secure world changes.
        expect_eq!(read_s2por_el1(), S2porEl1::empty());

        // Restore the original secure world state.
        let _ = helper([0, 0, 0])?;
    }

    Ok(())
}
