// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD, SVE and SME support.

#[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
mod simd_sel1;

use super::CpuExtension;

use crate::{
    aarch64::isb,
    context::{CpuContext, PerWorldContext, World},
};

use arm_sysregs::{
    CptrEl3, IdAa64smfr0El1, ScrEl3, SmcrEl3, read_cptr_el3, read_id_aa64pfr0_el1,
    read_id_aa64pfr1_el1, read_id_aa64smfr0_el1, write_cptr_el3, write_smcr_el3, write_zcr_el3,
};

/// Enables FP/SIMD register access for all worlds.
///
/// If `sel2` is enabled, S-EL2 is responsible for FP/SIMD context management.
/// Otherwise, the context management is performed in EL3.
pub struct Simd {
    /// This extension will save / restore NS FP registers only if this flag is set and `sel2` is
    /// disabled.
    /// This flag is used to not duplicate context management when SVE extension is enabled,
    /// as FP registers overlap with SVE state.
    #[allow(dead_code)]
    manage_ns_context: bool,
}

impl Simd {
    /// `manage_ns_context` should be true iff the Sve extension is not enabled for the platform.
    #[allow(unused)]
    pub const fn new(manage_ns_context: bool) -> Self {
        Self { manage_ns_context }
    }
}

impl CpuExtension for Simd {
    fn is_present(&self) -> bool {
        // FP is mandatory.
        true
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        // Allow FP/SIMD register accesses in every World.
        ctx.cptr_el3 -= CptrEl3::TFP;
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn save_context(&self, world: World) {
        use crate::platform::exception_free;

        if world == World::NonSecure && !self.manage_ns_context {
            return;
        }

        exception_free(|token| {
            simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].save();
        })
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn restore_context(&self, world: World) {
        use crate::platform::exception_free;

        if world == World::NonSecure && !self.manage_ns_context {
            return;
        }

        exception_free(|token| {
            simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].restore();
        })
    }
}

/// FEAT_SVE support.
///
/// Enables NS world SVE register access and configures the maximum SVE vector length.
///
/// TODO: Make it possible to enable SVE for SWd as well and handle SVE context switch if sel2 is
/// not enabled.
pub struct Sve {
    /// Limits the Effective Non-streaming SVE vector length to `vector_length` bits.
    vector_length: u64,
}

impl Sve {
    #[allow(unused)]
    pub const fn new(vector_length: u64) -> Self {
        assert!(
            vector_length % 128 == 0 && vector_length >= 128 && vector_length <= 2048,
            "Invalid SVE vector length"
        );
        Self { vector_length }
    }
}

impl CpuExtension for Sve {
    fn is_present(&self) -> bool {
        read_id_aa64pfr0_el1().is_feat_sve_present()
    }

    fn init(&self) {
        // Temporarily allow SVE register access, to configure the maximum SVE vector length.
        let cptr_el3 = read_cptr_el3();
        write_cptr_el3(cptr_el3 | CptrEl3::EZ);
        isb();

        // ZCR_EL3[3:0]:
        // Requests an Effective Non-streaming SVE vector length at EL3 of (LEN+1)*128 bits.
        write_zcr_el3(self.vector_length / 128 - 1);

        // Restore CPTR_EL3.
        write_cptr_el3(cptr_el3);
    }

    fn configure_per_world(&self, world: World, ctx: &mut PerWorldContext) {
        if world == World::NonSecure {
            // Allow NS world SVE register access.
            ctx.cptr_el3 |= CptrEl3::EZ;
        }
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn save_context(&self, world: World) {
        use crate::platform::exception_free;

        if world == World::NonSecure {
            exception_free(|token| {
                if self.is_present() {
                    simd_sel1::NS_SVE_CTX.get().borrow_mut(token).save();
                } else {
                    simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].save();
                }
            })
        }
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn restore_context(&self, world: World) {
        use crate::platform::exception_free;

        if world == World::NonSecure {
            exception_free(|token| {
                if self.is_present() {
                    simd_sel1::NS_SVE_CTX.get().borrow_mut(token).restore();
                } else {
                    simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].restore();
                }
            })
        }
    }
}

/// FEAT_SME support.
///
/// Enables NS world SME register access and configures the maximum Streaming SVE (SSVE) vector
/// length.
pub struct Sme {
    /// Limits the Effective Streaming SVE vector length to `vector_length` bits.
    vector_length: u64,
}

impl Sme {
    #[allow(unused)]
    pub const fn new(vector_length: u64) -> Self {
        assert!(
            vector_length % 128 == 0 && vector_length >= 128 && vector_length <= 2048,
            "Invalid SSVE vector length"
        );
        Self { vector_length }
    }
}

impl CpuExtension for Sme {
    fn is_present(&self) -> bool {
        read_id_aa64pfr1_el1().is_feat_sme_present()
    }

    fn init(&self) {
        // Temporarily allow SME register access, to configure the maximum SSVE vector length.
        let cptr_el3 = read_cptr_el3();
        write_cptr_el3(cptr_el3 | CptrEl3::ESM);
        isb();

        // Configure maximum SSVE vector length.
        let mut smcr_el3 = SmcrEl3::from_ssve_vector_len(self.vector_length);

        if read_id_aa64smfr0_el1().contains(IdAa64smfr0El1::FA64) {
            smcr_el3 |= SmcrEl3::FA64;
        }

        // Enable access to ZT0 registers if SME2 is present.
        if read_id_aa64pfr1_el1().is_feat_sme2_present() {
            smcr_el3 |= SmcrEl3::EZT0;
        }

        // Configure SMCR_EL3 for all worlds.
        write_smcr_el3(smcr_el3);

        // Restore CPTR_EL3.
        write_cptr_el3(cptr_el3);
    }

    fn configure_per_cpu(&self, world: World, context: &mut CpuContext) {
        if world == World::NonSecure {
            context.el3_state.scr_el3 |= ScrEl3::ENTP2;
        }
    }

    fn configure_per_world(&self, world: World, ctx: &mut PerWorldContext) {
        if world == World::NonSecure {
            // Allow NS world SME register access.
            ctx.cptr_el3 |= CptrEl3::ESM;
        }
    }
}
