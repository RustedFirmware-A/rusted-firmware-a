// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Reliability, Accessibility, Serviceability (RAS) extension.

#[cfg(not(feature = "sel2"))]
mod ras_sel1;
#[cfg(feature = "sel2")]
mod ras_sel2;

use super::CpuExtension;
use crate::context::World;

/// Enables context switching of the Reliability, Accessibility, Serviceability (RAS) extension
/// registers on world switch. If RAS features are used by lower ELs then this extension must be
/// enabled.
pub struct Ras;

impl CpuExtension for Ras {
    fn is_present(&self) -> bool {
        /* Assume that FEAT_RAS is present as it is mandatory from Armv8.2 */
        true
    }

    #[allow(unused)]
    fn save_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            ras_sel2::save_context(world);
            #[cfg(not(feature = "sel2"))]
            ras_sel1::save_context(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            #[cfg(feature = "sel2")]
            ras_sel2::restore_context(world);
            #[cfg(not(feature = "sel2"))]
            ras_sel1::restore_context(world);
        }
    }
}
