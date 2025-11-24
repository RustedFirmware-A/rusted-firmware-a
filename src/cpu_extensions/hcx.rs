// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_HCX introduces the Extended Hypervisor Configuration Register, HCRX_EL2, that provides
//! configuration controls for virtualization in addition to those provided by HCR_EL2, including
//! defining whether various operations are trapped to EL2.

#[cfg(feature = "sel2")]
mod hcx_sel2;

use super::CpuExtension;
use crate::context::{PerWorldContext, World};
use arm_sysregs::{HcrxEl2, ScrEl3, read_id_aa64mmfr1_el1, write_hcrx_el2};

pub struct Hcx;

impl CpuExtension for Hcx {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr1_el1().is_feat_hcx_present()
    }

    fn init(&self) {
        // Initialize register HCRX_EL2 to all-zero.
        // As the value of HCRX_EL2 is UNKNOWN on reset, there is a chance that this can lead to
        // unexpected behavior in lower ELs that have not been updated since the introduction of
        // this feature if not properly initialized, especially when it comes to those bits that
        // enable/disable traps.
        // SAFETY: 0 is a valid value.
        unsafe {
            write_hcrx_el2(HcrxEl2::empty());
        }
    }

    fn configure_per_world(&self, _world: World, context: &mut PerWorldContext) {
        context.scr_el3 |= ScrEl3::HXEN;
    }

    #[cfg(feature = "sel2")]
    #[allow(unused)]
    fn save_context(&self, world: World) {
        if self.is_present() {
            hcx_sel2::save_context(world);
        }
    }

    #[cfg(feature = "sel2")]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            hcx_sel2::restore_context(world);
        }
    }
}
