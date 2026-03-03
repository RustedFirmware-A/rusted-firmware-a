// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests SIMD context switch.

use crate::{
    framework::{TestHelperProxy, TestResult, normal_world_test},
    tests::simd_helper::{
        Phase, SimdVectors, overwrite_simd, read_simd, test_simd_context_switch_helper,
    },
};

normal_world_test!(
    test_simd_context_switch,
    helper = test_simd_context_switch_helper
);
/// Checks if SIMD vector registers' state is preserved across world switches.
fn test_simd_context_switch(helper: &TestHelperProxy) -> TestResult {
    // NSWd overwrites SIMD vector registers with 1, 3, 5, .. (consecutive odd numbers).
    let nswd_simd_state: SimdVectors = core::array::from_fn(|i| (2 * i + 1) as u128);
    overwrite_simd(&nswd_simd_state);
    assert_eq!(
        read_simd(),
        nswd_simd_state,
        "NSWd failed to overwrite SIMD registers."
    );

    // Switch to the SWd side of the test.
    helper(Phase::SWdOverwriteSIMD.into())?;

    // Make sure the world switch did not destroy NSWd SIMD state.
    assert_eq!(
        read_simd(),
        nswd_simd_state,
        "NSWd SIMD should be preserved across world switches."
    );

    // Switch to the SWd side of the test.
    helper(Phase::SWdCheckSIMD.into())?;

    Ok(())
}
