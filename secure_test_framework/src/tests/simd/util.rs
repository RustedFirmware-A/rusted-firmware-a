// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Shared helper for SIMD and SVE context switch tests.

use crate::{
    framework::{TestHelperRequest, TestHelperResponse},
    util::current_el,
};
use arm_sysregs::read_id_aa64pfr0_el1;
use core::arch::asm;
use log::warn;
use spin::mutex::SpinMutex;

pub type SimdVectors = [u128; 32];

// SVE vectors can be up to 2048 bits (256 bytes).
// Predicates can be up to 256 bits (32 bytes).
// We use a flat array to avoid issues with fixed-size sub-arrays when VL is variable.
pub type SveVectors = [u128; 32 * 16];
pub type SvePredicates = [u128; 32];

// Use static variables for SVE state to avoid stack overflow.
pub static READ_BUFFER_VECTORS: SpinMutex<SveVectors> = SpinMutex::new([0; 32 * 16]);
pub static READ_BUFFER_PREDICATES: SpinMutex<SvePredicates> = SpinMutex::new([0; 32]);

/// Generic response to just indicate that the secure world helper
/// phase has been executed successfully.
pub const PHASE_SUCCESS: TestHelperResponse = [0, 0, 0, 0];

/// Phase of a context switch test to be executed in TestHelperProxy.
#[repr(u64)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Phase {
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
pub fn read_simd() -> SimdVectors {
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
pub fn overwrite_simd(regs: &SimdVectors) {
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

pub fn is_sve_present() -> bool {
    read_id_aa64pfr0_el1().is_feat_sve_present()
}

pub fn get_vl_bytes() -> usize {
    let vl: u64;
    // SAFETY: We only call this if SVE is present.
    unsafe {
        asm!(
            ".arch_extension sve",
            "rdvl {0}, #1",
            out(reg) vl,
        );
    }
    vl as usize
}

/// Returns current state of SVE vector registers.
pub fn read_sve_vectors(dest_buf: &mut SveVectors) {
    let dest = dest_buf.as_mut_ptr();

    // SAFETY: `dest` is a 16B aligned valid pointer to a 32 * 256 byte array.
    unsafe {
        asm!(
            ".arch_extension sve",
            "str z0, [{dest}, #0, MUL VL]",
            "str z1, [{dest}, #1, MUL VL]",
            "str z2, [{dest}, #2, MUL VL]",
            "str z3, [{dest}, #3, MUL VL]",
            "str z4, [{dest}, #4, MUL VL]",
            "str z5, [{dest}, #5, MUL VL]",
            "str z6, [{dest}, #6, MUL VL]",
            "str z7, [{dest}, #7, MUL VL]",
            "str z8, [{dest}, #8, MUL VL]",
            "str z9, [{dest}, #9, MUL VL]",
            "str z10, [{dest}, #10, MUL VL]",
            "str z11, [{dest}, #11, MUL VL]",
            "str z12, [{dest}, #12, MUL VL]",
            "str z13, [{dest}, #13, MUL VL]",
            "str z14, [{dest}, #14, MUL VL]",
            "str z15, [{dest}, #15, MUL VL]",
            "str z16, [{dest}, #16, MUL VL]",
            "str z17, [{dest}, #17, MUL VL]",
            "str z18, [{dest}, #18, MUL VL]",
            "str z19, [{dest}, #19, MUL VL]",
            "str z20, [{dest}, #20, MUL VL]",
            "str z21, [{dest}, #21, MUL VL]",
            "str z22, [{dest}, #22, MUL VL]",
            "str z23, [{dest}, #23, MUL VL]",
            "str z24, [{dest}, #24, MUL VL]",
            "str z25, [{dest}, #25, MUL VL]",
            "str z26, [{dest}, #26, MUL VL]",
            "str z27, [{dest}, #27, MUL VL]",
            "str z28, [{dest}, #28, MUL VL]",
            "str z29, [{dest}, #29, MUL VL]",
            "str z30, [{dest}, #30, MUL VL]",
            "str z31, [{dest}, #31, MUL VL]",
            dest = in(reg) dest,
        );
    }
}

/// Overwrites SVE vector registers with provided values.
pub fn overwrite_sve_vectors(src_buf: &SveVectors) {
    let src = src_buf.as_ptr();

    // SAFETY: `src` is a 16B aligned valid pointer to a 32 * 256 byte array.
    unsafe {
        asm!(
            ".arch_extension sve",
            "ldr z0, [{src}, #0, MUL VL]",
            "ldr z1, [{src}, #1, MUL VL]",
            "ldr z2, [{src}, #2, MUL VL]",
            "ldr z3, [{src}, #3, MUL VL]",
            "ldr z4, [{src}, #4, MUL VL]",
            "ldr z5, [{src}, #5, MUL VL]",
            "ldr z6, [{src}, #6, MUL VL]",
            "ldr z7, [{src}, #7, MUL VL]",
            "ldr z8, [{src}, #8, MUL VL]",
            "ldr z9, [{src}, #9, MUL VL]",
            "ldr z10, [{src}, #10, MUL VL]",
            "ldr z11, [{src}, #11, MUL VL]",
            "ldr z12, [{src}, #12, MUL VL]",
            "ldr z13, [{src}, #13, MUL VL]",
            "ldr z14, [{src}, #14, MUL VL]",
            "ldr z15, [{src}, #15, MUL VL]",
            "ldr z16, [{src}, #16, MUL VL]",
            "ldr z17, [{src}, #17, MUL VL]",
            "ldr z18, [{src}, #18, MUL VL]",
            "ldr z19, [{src}, #19, MUL VL]",
            "ldr z20, [{src}, #20, MUL VL]",
            "ldr z21, [{src}, #21, MUL VL]",
            "ldr z22, [{src}, #22, MUL VL]",
            "ldr z23, [{src}, #23, MUL VL]",
            "ldr z24, [{src}, #24, MUL VL]",
            "ldr z25, [{src}, #25, MUL VL]",
            "ldr z26, [{src}, #26, MUL VL]",
            "ldr z27, [{src}, #27, MUL VL]",
            "ldr z28, [{src}, #28, MUL VL]",
            "ldr z29, [{src}, #29, MUL VL]",
            "ldr z30, [{src}, #30, MUL VL]",
            "ldr z31, [{src}, #31, MUL VL]",
            src = in(reg) src,
        );
    }
}

/// Returns current state of SVE predicate registers.
pub fn read_sve_predicates(dest_buf: &mut SvePredicates) {
    let dest = dest_buf.as_mut_ptr();

    // SAFETY: `dest` is a 16B aligned valid pointer to a 32 * 16 byte array.
    unsafe {
        asm!(
            ".arch_extension sve",
            "str p0, [{dest}, #0, MUL VL]",
            "str p1, [{dest}, #1, MUL VL]",
            "str p2, [{dest}, #2, MUL VL]",
            "str p3, [{dest}, #3, MUL VL]",
            "str p4, [{dest}, #4, MUL VL]",
            "str p5, [{dest}, #5, MUL VL]",
            "str p6, [{dest}, #6, MUL VL]",
            "str p7, [{dest}, #7, MUL VL]",
            "str p8, [{dest}, #8, MUL VL]",
            "str p9, [{dest}, #9, MUL VL]",
            "str p10, [{dest}, #10, MUL VL]",
            "str p11, [{dest}, #11, MUL VL]",
            "str p12, [{dest}, #12, MUL VL]",
            "str p13, [{dest}, #13, MUL VL]",
            "str p14, [{dest}, #14, MUL VL]",
            "str p15, [{dest}, #15, MUL VL]",
            dest = in(reg) dest,
        );
    }
}

/// Overwrites SVE predicate registers with provided values.
pub fn overwrite_sve_predicates(src_buf: &SvePredicates) {
    let src = src_buf.as_ptr();

    // SAFETY: `src` is a 16B aligned valid pointer to a 32 * 16 byte array.
    unsafe {
        asm!(
            ".arch_extension sve",
            "ldr p0, [{src}, #0, MUL VL]",
            "ldr p1, [{src}, #1, MUL VL]",
            "ldr p2, [{src}, #2, MUL VL]",
            "ldr p3, [{src}, #3, MUL VL]",
            "ldr p4, [{src}, #4, MUL VL]",
            "ldr p5, [{src}, #5, MUL VL]",
            "ldr p6, [{src}, #6, MUL VL]",
            "ldr p7, [{src}, #7, MUL VL]",
            "ldr p8, [{src}, #8, MUL VL]",
            "ldr p9, [{src}, #9, MUL VL]",
            "ldr p10, [{src}, #10, MUL VL]",
            "ldr p11, [{src}, #11, MUL VL]",
            "ldr p12, [{src}, #12, MUL VL]",
            "ldr p13, [{src}, #13, MUL VL]",
            "ldr p14, [{src}, #14, MUL VL]",
            "ldr p15, [{src}, #15, MUL VL]",
            src = in(reg) src,
        );
    }
}

/// The secure world side of the SIMD context switch test.
pub fn test_simd_context_switch_helper(
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
