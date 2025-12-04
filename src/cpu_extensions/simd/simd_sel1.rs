// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD and SVE context management for when Secure EL2 is not enabled.
use crate::{
    aarch64::isb,
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

pub static NS_SVE_CTX: PerCoreState<SveCpuContext> = PerCore::new(
    [const { ExceptionLock::new(RefCell::new(SveCpuContext::EMPTY)) }; PlatformImpl::CORE_COUNT],
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

#[repr(C)]
pub struct SveCpuContext {
    vectors: [[u128; 16]; 32], // TODO: [u128; 16] is the MAX capacity. Allow platforms to adjust.
    predicates: [u128; 256 / 8], // TODO: Adjust to [u128; SVE_VECTOR_LEN_BYTES / 8]
    ffr: [u8; 256 / 8],        // TODO: Adjust to [u8; SVE_VECTOR_LEN_BYTES / 8]
    fpsr: u64,
    fpcr: u64,
}

impl SveCpuContext {
    const EMPTY: Self = Self {
        vectors: [[0; 16]; 32],
        predicates: [0; 256 / 8],
        ffr: [0; 256 / 8],
        fpsr: 0,
        fpcr: 0,
    };

    fn save_ffr(&mut self) {
        // Get a mutable pointer to the start of the ffr storage.
        let dest = self.ffr.as_mut_ptr();

        // SAFETY: `dest` is a 16B aligned valid pointer to an array that can hold SVE ffr
        // registers of maximum length.
        unsafe {
            asm!(
                ".arch_extension sve",
                "rdffr p0.b",
                "str p0, [{dest}]",
                ".arch_extension nosve",
                dest = in(reg) dest,
                options(nostack, preserves_flags)
            )
        }
    }

    fn restore_ffr(&self) {
        // Get a pointer to the start of the ffr storage.
        let src = self.ffr.as_ptr();

        // SAFETY: `src` is a 16B aligned valid pointer to an array that can hold SVE predicate
        // registers of maximum length.
        unsafe {
            asm!(
                ".arch_extension sve",
                "ldr p0, [{src}]",
                "wrffr p0.b",
                ".arch_extension nosve",
                src = in(reg) src,
                options(nostack, readonly, preserves_flags)
            )
        }
    }

    fn save_predicates(&mut self) {
        // Get a mutable pointer to the start of the predicates storage.
        let dest = self.predicates.as_mut_ptr();

        // SAFETY: `dest` is a 16B aligned valid pointer to an array that can hold SVE predicate
        // registers of maximum length.
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
                ".arch_extension nosve",
                // inout because str instructions advance the pointer.
                dest = inout(reg) dest => _,
                options(nostack, preserves_flags)
            )
        }
    }

    fn restore_predicates(&self) {
        // Get a pointer to the start of the predicate storage.
        let src = self.predicates.as_ptr();

        // SAFETY: `src` is a 16B aligned valid pointer to an array that can hold SVE predicate
        // registers of maximum length.
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
                ".arch_extension nosve",
                // inout because ldr instructions advance the pointer.
                src = inout(reg) src => _,
                options(nostack, readonly, preserves_flags)
            )
        }
    }

    /// Saves the 32 SVE vector registers using optimized store instruction.
    fn save_vectors(&mut self) {
        // Get a mutable pointer to the start of the vector storage.
        let dest = self.vectors.as_mut_ptr();

        // SAFETY: `dest` is a 16B aligned valid pointer to an array that can hold SVE vectors of
        // maximum length.
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
                ".arch_extension nosve",
                // inout because stp instructions advance the pointer.
                dest = inout(reg) dest => _,
                options(nostack, preserves_flags)
            )
        }
    }

    /// Restores the 32 SVE vector registers using optimized load instruction.
    fn restore_vectors(&self) {
        // Get a pointer to the start of the vector storage.
        let src = self.vectors.as_ptr();

        // SAFETY: `src` is a 16B aligned valid pointer to an array that can hold SVE vectors of
        // maximum length.
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
                ".arch_extension nosve",
                // inout because ldr instructions advance the pointer.
                src = inout(reg) src => _,
                options(nostack, readonly, preserves_flags)
            );
        }
    }

    /// Saves FP state registers.
    fn save_fp_state(&mut self) {
        let fpsr_value;
        let fpcr_value;

        // SAFETY: This asm only reads the fpsr and fpcr to registers
        unsafe {
            asm!(
                ".arch_extension fp",
                "mrs {fpsr_value}, fpsr",
                "mrs {fpcr_value}, fpcr",
                ".arch_extension nofp",
                fpsr_value = out(reg) fpsr_value,
                fpcr_value = out(reg) fpcr_value,
                options(nostack, nomem, preserves_flags)
            )
        }

        self.fpsr = fpsr_value;
        self.fpcr = fpcr_value;
    }

    /// Restores FP state registers.
    fn restore_fp_state(&self) {
        // SAFETY: This asm only stores the fpsr and fpcr into registers.
        unsafe {
            asm!(
                ".arch_extension fp",
                "msr fpsr, {fpsr_value}",
                "msr fpcr, {fpcr_value}",
                ".arch_extension nofp",
                fpsr_value = in(reg) self.fpsr,
                fpcr_value = in(reg) self.fpcr,
                // Option `preserves_flags` cannot be set as it assumes that the asm block does not
                // modify `fpsr` which is restored here.
                options(nostack, nomem)
            );
        }
    }

    pub fn save(&mut self) {
        self.save_predicates();
        isb(); // Note: predicates must be saved before ffr.
        self.save_ffr();
        self.save_vectors();
        self.save_fp_state();
    }

    pub fn restore(&self) {
        self.restore_ffr();
        isb(); // Note: predicates must be restored after ffr.
        self.restore_predicates();
        self.restore_vectors();
        self.restore_fp_state();
    }
}
