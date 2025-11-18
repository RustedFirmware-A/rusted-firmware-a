// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD context management for when Secure EL2 is not enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld},
    platform::{Platform, PlatformImpl},
};
use core::{arch::asm, cell::RefCell};
use percore::{ExceptionLock, PerCore};

pub static SIMD_CTX: PerCoreState<PerWorld<SimdCpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [SimdCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

#[repr(C)]
pub struct SimdCpuContext {
    vectors: [u128; 32],
    fpsr: u64,
    fpcr: u64,
}

impl SimdCpuContext {
    const EMPTY: Self = Self {
        vectors: [0; 32],
        fpsr: 0,
        fpcr: 0,
    };

    /// Saves the 32 SIMD/FP (Q) registers using the optimized ARM64 STP (Store Pair) instruction
    /// with post-indexing and saves the FP state registers.
    pub fn save(&mut self) {
        // Get a mutable pointer to the start of the vector storage.
        let dest = self.vectors.as_mut_ptr();
        let fpsr_value;
        let fpcr_value;

        // SAFETY: `dest` is a 16B aligned valid pointer to a 32 * 32 byte array.
        unsafe {
            asm!(
                // The instructions save 32 bytes (Qx and Qy) per line and advance the pointer by
                // 32 bytes.
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

                "mrs {fpsr_value}, fpsr",
                "mrs {fpcr_value}, fpcr",
                ".arch_extension nofp",
                // inout because stp instructions advance the pointer.
                dest = inout(reg) dest => _,
                fpsr_value = out(reg) fpsr_value,
                fpcr_value = out(reg) fpcr_value,
                options(nostack, preserves_flags)
            );
        }

        self.fpsr = fpsr_value;
        self.fpcr = fpcr_value;
    }

    /// Restores the 32 SIMD/FP (Q) registers using the optimized ARM64 LDP (Load Pair) instruction
    /// with post-indexing and restores the FP state registers.
    pub fn restore(&self) {
        // Get a pointer to the start of the vector storage.
        let src = self.vectors.as_ptr();

        // SAFETY: `src` is a 16B aligned valid pointer to a 32 * 32 byte array.
        unsafe {
            asm!(
                // The instructions load 32 bytes (Qx and Qy) per line and advance the pointer by
                // 32 bytes.
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

                "msr fpsr, {fpsr_value}",
                "msr fpcr, {fpcr_value}",
                ".arch_extension nofp",
                // inout because ldp instructions advance the pointer.
                src = inout(reg) src => _,
                fpsr_value = in(reg) self.fpsr,
                fpcr_value = in(reg) self.fpcr,
                options(nostack, readonly)
            );
        }
    }
}
