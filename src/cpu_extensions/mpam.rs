// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Memory System Resource Partitioning and Monitoring (MPAM) extension
//!
//! MPAM extends the ability for software to co-manage runtime resource allocation of memory system
//! components such as caches,interconnects, and memory controllers. These are resources that
//! otherwise would not be controllable by software at this granularity.
//!
//! MPAM achieves this by enabling supervisory software such as an OS or Hypervisor to assign a
//! unique partition identifier to instruction accesses and data memory accesses for each VM or
//! application. These uniquely assigned partition identifiers accompany the memory accesses
//! throughout their lifetime in the memory system. Memory system components use partition
//! identifiers to configure the allocation of resources to a particular VM or application.

#[cfg(feature = "sel2")]
mod mpam_sel2;

use super::CpuExtension;

use crate::context::{PerWorldContext, World};

use arm_sysregs::{Mpam3El3, read_id_aa64pfr0_el1};

/// FEAT_MPAM support
///
/// Enables MPAM configuration adn disables MPAM system register traps for NS and Realm worlds.
pub struct Mpam;

impl CpuExtension for Mpam {
    fn is_present(&self) -> bool {
        read_id_aa64pfr0_el1().is_feat_mpam_present()
    }

    fn configure_per_world(&self, world: World, ctx: &mut PerWorldContext) {
        if world != World::Secure {
            // Enable MPAM configuration and clear the default TRAPLOWER=1 for worlds other than
            // SWd.
            ctx.mpam3_el3 = Mpam3El3::MPAMEN
        }
    }

    #[cfg(feature = "sel2")]
    fn save_context(&self, world: World) {
        if self.is_present() {
            mpam_sel2::save_context(world);
        }
    }

    #[cfg(feature = "sel2")]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            mpam_sel2::restore_context(world);
        }
    }
}
