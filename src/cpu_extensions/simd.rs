// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD and SVE support.

#[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
mod simd_sel1;

use super::CpuExtension;

use crate::{
    aarch64::isb,
    context::{PerWorldContext, World},
};

use arm_sysregs::{CptrEl3, read_cptr_el3, read_id_aa64pfr0_el1, write_cptr_el3, write_zcr_el3};

/// Enables FP/SIMD register access for all worlds.
///
/// If `sel2` is enabled, S-EL2 is responsible for FP/SIMD context management.
/// Otherwise, the context management is performed in EL3.
pub struct Simd;

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

        exception_free(|token| {
            let ctx = &mut simd_sel1::SIMD_CTX.get().borrow_mut(token)[world];

            ctx.save();
        })
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn restore_context(&self, world: World) {
        use crate::platform::exception_free;

        exception_free(|token| {
            let ctx = &simd_sel1::SIMD_CTX.get().borrow_mut(token)[world];

            ctx.restore();
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
}
