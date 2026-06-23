// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FEAT_SCTLR2 architecture extension.
//!
//! Tests if the SCTLR2_EL2 register is accessible from EL2. All bits in SCTLR2_EL2 depend on some
//! optional feature. Currently none of these are supported by RF-A, all bits are RES0 and writes
//! are ignored, so preservation of the register contents cannot be tested.

use crate::framework::{TestError, TestResult, normal_world_test};
use arm_sysregs::{read_id_aa64mmfr3_el1, read_sctlr2_el2};
use log::debug;

normal_world_test!(test_sctlr2);

/// Checks that SCTLR2_EL2 is accessible from EL2.
fn test_sctlr2() -> TestResult {
    if !read_id_aa64mmfr3_el1().is_feat_sctlr2_present() {
        debug!("FEAT_SCTLR2 not present, skipping test.");
        return Err(TestError::Ignored);
    }

    let _ = read_sctlr2_el2();

    Ok(())
}
