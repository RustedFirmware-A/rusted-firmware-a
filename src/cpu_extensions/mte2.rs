// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Memory Tagging Extension

#[cfg(not(feature = "sel2"))]
mod mte2_sel1;
#[cfg(feature = "sel2")]
mod mte2_sel2;

use super::CpuExtension;
use crate::context::{CpuContext, World};
use arm_sysregs::{ScrEl3, read_id_aa64pfr1_el1};

/// Memory Tagging Extension
///
/// Configures the Memory Tagging Extension (FEAT_MTE2) to enable the Non-secure and Secure worlds
/// to use it.
///
/// FEAT_MTE2 provides architectural support for runtime, always-on detection of various classes of
/// memory error to aid with software debugging to eliminate vulnerabilities arising from
/// memory-unsafe languages.
pub struct MemoryTagging;

impl CpuExtension for MemoryTagging {
    fn is_present(&self) -> bool {
        read_id_aa64pfr1_el1().is_feat_mte2_present()
    }

    fn configure_per_cpu(&self, world: World, context: &mut CpuContext) {
        // Allow access to Allocation Tags for Non-secure and Secure worlds when FEAT_MTE2 is
        // implemented.
        if world == World::NonSecure || world == World::Secure {
            context.el3_state.scr_el3 |= ScrEl3::ATA;
        }
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            mte2_sel2::save_context(world);
            #[cfg(not(feature = "sel2"))]
            mte2_sel1::save_context(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            mte2_sel2::restore_context(world);
            #[cfg(not(feature = "sel2"))]
            mte2_sel1::restore_context(world);
        }
    }
}
