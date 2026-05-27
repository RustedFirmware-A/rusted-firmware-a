// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Floating-point Mode Register extension.

use super::CpuExtension;
use crate::context::{PerWorldContext, World};
use arm_sysregs::{ScrEl3, read_id_aa64pfr2_el1};

/// FEAT_FPMR introduces the Floating-point Mode Register, FPMR.
///
/// Enables access to the FPMR registers from the Non-secure world. Since
/// FEAT_FPMR requires FP/SIMD support, the `Simd` CPU extension is responsible
/// for clearing `CPTR_EL3.TFP`, which is also required for this feature.
pub struct Fpmr;

impl CpuExtension for Fpmr {
    fn is_present(&self) -> bool {
        read_id_aa64pfr2_el1().is_feat_fpmr_present()
    }

    fn configure_per_world(&self, world: World, context: &mut PerWorldContext) {
        if world == World::NonSecure {
            context.scr_el3 |= ScrEl3::ENFPM;
        }
    }
}
