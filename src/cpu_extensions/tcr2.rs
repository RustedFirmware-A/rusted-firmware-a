// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 introduces the TCR2_ELx registers which provide top-level control of the EL1&0
//! and EL2&0 translation regimes respectively.

#[cfg(not(feature = "sel2"))]
mod tcr2_sel1;
#[cfg(feature = "sel2")]
mod tcr2_sel2;

use super::CpuExtension;
use crate::context::{PerWorldContext, World};
use arm_sysregs::{ScrEl3, read_id_aa64mmfr3_el1};

/// Enables access to the TCR2_ELx registers at lower ELs, along with context switching of those
/// registers on world switch.
pub struct Tcr2;

impl CpuExtension for Tcr2 {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_tcr2_present()
    }

    fn configure_per_world(&self, _world: World, context: &mut PerWorldContext) {
        // Enable access to TCR2_ELx registers at lower ELs.
        context.scr_el3 |= ScrEl3::TCR2EN;
    }

    #[allow(unused)]
    fn save_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            tcr2_sel2::save_context(world);
            #[cfg(not(feature = "sel2"))]
            tcr2_sel1::save_context(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            tcr2_sel2::restore_context(world);
            #[cfg(not(feature = "sel2"))]
            tcr2_sel1::restore_context(world);
        }
    }
}
