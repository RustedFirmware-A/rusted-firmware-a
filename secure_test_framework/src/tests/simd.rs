// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests SIMD context switch.

// TODO: Make it possible for a platform to enable SVE in STF and test NS SVE context switch.

use crate::{
    framework::{
        TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult, normal_world_test,
    },
    util::current_el,
};
use core::arch::asm;
use log::warn;

type SimdVectors = [u128; 32];

/// Generic response to just indicate that the secure world helper
/// phase has been executed successfully.
const PHASE_SUCCESS: TestHelperResponse = [0, 0, 0, 0];

/// Phase of a SIMD context switch test to be executed in TestHelperProxy.
#[repr(u64)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Phase {
    /// SWd overwrites its SIMD vector registers.
    /// NSWd then checks if its SIMD vectors registers are preserved across world switches.
    SWdOverwriteSIMD,
    /// NSWd overwrites its SIMD vector registers.
    /// SWd checks if its SIMD vectors registers are preserved across world switches.
    SWdCheckSIMD,
}

impl TryFrom<TestHelperRequest> for Phase {
    type Error = ();

    fn try_from(value: TestHelperRequest) -> Result<Self, Self::Error> {
        match value {
            [phase, ..] if phase == Phase::SWdOverwriteSIMD as u64 => Ok(Phase::SWdOverwriteSIMD),
            [phase, ..] if phase == Phase::SWdCheckSIMD as u64 => Ok(Phase::SWdCheckSIMD),
            _ => Err(()),
        }
    }
}

impl From<Phase> for TestHelperRequest {
    fn from(phase: Phase) -> Self {
        [phase as u64, 0, 0]
    }
}

/// Returns current state of SIMD vector registers.
fn read_simd() -> SimdVectors {
    let mut regs: SimdVectors = [0; 32];

    // The instructions save 32 bytes (Qx and Qy) per line and advance the pointer by 32.
    // SAFETY: `dest` is a 16B aligned valid pointer to a 32 * 32 byte array.
    unsafe {
        asm!(
            ".arch_extension fp",
            "stp q0, q1, [{dest}], #32",
            "stp q2, q3, [{dest}], #32",
            "stp q4, q5, [{dest}], #32",
            "stp q6, q7, [{dest}], #32",
            "stp q8, q9, [{dest}], #32",
            "stp q10, q11, [{dest}], #32",
            "stp q12, q13, [{dest}], #32",
            "stp q14, q15, [{dest}], #32",
            "stp q16, q17, [{dest}], #32",
            "stp q18, q19, [{dest}], #32",
            "stp q20, q21, [{dest}], #32",
            "stp q22, q23, [{dest}], #32",
            "stp q24, q25, [{dest}], #32",
            "stp q26, q27, [{dest}], #32",
            "stp q28, q29, [{dest}], #32",
            "stp q30, q31, [{dest}], #32",
            ".arch_extension nofp",
            // inout because stp instructions advance the pointer.
            dest = inout(reg) regs.as_mut_ptr() => _,
        );
    }

    regs
}

/// Overwrites SIMD vector registers with provided values.
fn overwrite_simd(regs: &SimdVectors) {
    // The instructions load 32 bytes (Qx and Qy) per line and advance the pointer by 32.
    // SAFETY: `src` is a 16B aligned valid pointer to a 32 * 32 byte array.
    unsafe {
        asm!(
            ".arch_extension fp",
            "ldp q0, q1, [{src}], #32",
            "ldp q2, q3, [{src}], #32",
            "ldp q4, q5, [{src}], #32",
            "ldp q6, q7, [{src}], #32",
            "ldp q8, q9, [{src}], #32",
            "ldp q10, q11, [{src}], #32",
            "ldp q12, q13, [{src}], #32",
            "ldp q14, q15, [{src}], #32",
            "ldp q16, q17, [{src}], #32",
            "ldp q18, q19, [{src}], #32",
            "ldp q20, q21, [{src}], #32",
            "ldp q22, q23, [{src}], #32",
            "ldp q24, q25, [{src}], #32",
            "ldp q26, q27, [{src}], #32",
            "ldp q28, q29, [{src}], #32",
            "ldp q30, q31, [{src}], #32",
            ".arch_extension nofp",
            // inout because ldp instructions advance the pointer.
            src = inout(reg) regs.as_ptr() => _,
        );
    }
}

/// The secure world side of the SIMD context switch test.
fn test_simd_context_switch_helper(
    ns_world_request: TestHelperRequest,
) -> Result<TestHelperResponse, ()> {
    if current_el() == 2 {
        // TODO: implement SIMD context management in STF if the secure component is running in
        // EL2.
        warn!("Skipping SWd context switch side of the test.");
        return Ok(PHASE_SUCCESS);
    }

    // SWd will overwrite its SIMD vector registers with 0, 2, 4, .. (consecutive even numbers).
    let swd_simd_state: SimdVectors = core::array::from_fn(|i| 2 * i as u128);

    match Phase::try_from(ns_world_request)? {
        Phase::SWdOverwriteSIMD => {
            // SWd overwrites its SIMD vector registers.
            overwrite_simd(&swd_simd_state);
            assert_eq!(
                read_simd(),
                swd_simd_state,
                "SWd failed to overwrite SIMD registers."
            );
        }
        Phase::SWdCheckSIMD => {
            // Make sure the world switch did not destroy SWd SIMD state.
            assert_eq!(
                read_simd(),
                swd_simd_state,
                "SWd SIMD should be preserved across world switches."
            );
        }
    }

    // SWd side of the test passed. NSWd ignores the returned values.
    Ok(PHASE_SUCCESS)
}

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
