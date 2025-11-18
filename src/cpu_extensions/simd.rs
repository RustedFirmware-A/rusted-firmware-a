// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD support.

#[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
mod simd_sel1;

use super::CpuExtension;

use crate::context::{PerWorldContext, World};

use arm_sysregs::CptrEl3;

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
