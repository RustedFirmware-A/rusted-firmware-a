// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FEAT_S1PIE and FEAT_S2PIE architecture extension.

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
            read_id_aa64mmfr3_el1, read_pir_el1, read_pire0_el1, write_pir_el1, write_pire0_el1,
        },
        registers::{PirEl1, Pire0El1},
    },
    el2::{
        accessors::{
            read_pir_el2, read_pire0_el2, read_s2pir_el2, write_pir_el2, write_pire0_el2,
            write_s2pir_el2,
        },
        registers::{PirEl2, Pire0El2, S2pirEl2},
    },
};

normal_world_test!(test_s1pie, helper = test_s1pie_helper);

/// Sets the PIRE0_ELx and PIR_ELx registers in the secure world. It returns the current EL of the
/// secure world component, so the normal world test can check whether the corresponding registers
/// have been preserved.
fn test_s1pie_helper([pire0, pir, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    match current_el() {
        1 => {
            // Safety: FEAT_S1PIE is supported
            unsafe {
                write_pire0_el1(Pire0El1::from_bits_retain(pire0));
                write_pir_el1(PirEl1::from_bits_retain(pir));
            }
        }
        2 => {
            // Safety: FEAT_S1PIE is supported
            unsafe {
                write_pire0_el2(Pire0El2::from_bits_retain(pire0));
                write_pir_el2(PirEl2::from_bits_retain(pir));
            }
        }
        _ => return Err(()),
    }

    Ok([current_el().into(), 0, 0, 0])
}

/// Uses the helper to set S1PIE related system registers in the secure world, and checks that the
/// normal world context has been preserved. The registers are expected to be set to zero during
/// boot.
fn test_s1pie(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr3_el1().is_feat_s1pie_present() {
        return Err(TestError::Ignored);
    }

    expect_eq!(read_pire0_el1(), Pire0El1::empty());
    expect_eq!(read_pir_el1(), PirEl1::empty());
    expect_eq!(read_pire0_el2(), Pire0El2::empty());
    expect_eq!(read_pir_el2(), PirEl2::empty());

    let [secure_el, ..] = helper([1, 1, 0])?;

    // Verify that S1PIE registers in normal world are not affected by secure world changes.
    match secure_el {
        1 => {
            expect_eq!(read_pire0_el1(), Pire0El1::empty());
            expect_eq!(read_pir_el1(), PirEl1::empty());
        }
        2 => {
            expect_eq!(read_pire0_el2(), Pire0El2::empty());
            expect_eq!(read_pir_el2(), PirEl2::empty());
        }
        _ => return Err(TestError::Failed),
    }

    // Restore the original secure world state.
    let _ = helper([0, 0, 0])?;

    Ok(())
}

normal_world_test!(test_s2pie, helper = test_s2pie_helper);

/// Sets the S2PIR_EL2 register in the secure world if running in EL2. It returns the current EL of
/// the secure world component.
fn test_s2pie_helper([s2pir, ..]: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    if current_el() == 2 {
        // Safety: FEAT_S2PIE is supported
        unsafe {
            write_s2pir_el2(S2pirEl2::from_bits_retain(s2pir));
        }
    }

    Ok([current_el().into(), 0, 0, 0])
}

/// Uses the helper to set S2PIE related system registers in the secure world, and checks that the
/// normal world context has been preserved if the secure world helper is deployed to EL2. The
/// registers are expected to be set to zero during boot.
fn test_s2pie(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64mmfr3_el1().is_feat_s2pie_present() {
        return Err(TestError::Ignored);
    }

    expect_eq!(read_s2pir_el2(), S2pirEl2::empty());

    let [secure_el, ..] = helper([1, 0, 0])?;

    if secure_el == 2 {
        // Verify that S2PIE registers in normal world are not affected by secure world changes.
        expect_eq!(read_s2pir_el2(), S2pirEl2::empty());

        // Restore the original secure world state.
        let _ = helper([0, 0, 0])?;
    }

    Ok(())
}
