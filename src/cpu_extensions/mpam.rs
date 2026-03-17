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

#[cfg(feature = "sel2")]
use self::mpam_sel2::MpamCpuContext;
use super::CpuExtension;
#[cfg(feature = "sel2")]
use crate::context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld};
use crate::{
    context::{PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{Mpam3El3, read_id_aa64pfr0_el1};
#[cfg(feature = "sel2")]
use core::cell::RefCell;
use core::marker::PhantomData;
#[cfg(feature = "sel2")]
use percore::{ExceptionLock, PerCore};

/// FEAT_MPAM support
///
/// Enables MPAM configuration and disables MPAM system register traps for NS and Realm worlds.
pub struct Mpam<const CORE_COUNT: usize, PlatformImpl: Platform> {
    #[cfg(feature = "sel2")]
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<MpamCpuContext>>,
    _platform: PhantomData<PlatformImpl>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Mpam<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the MPAM CPU extension.
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "sel2")]
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [MpamCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
            _platform: PhantomData,
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Mpam<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Mpam<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        mpam_is_present()
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
            self.save_el2_context(world);
        }
    }

    #[cfg(feature = "sel2")]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            self.restore_el2_context(world);
        }
    }
}

/// Returns whether MPAM is supported on the system.
pub fn mpam_is_present() -> bool {
    read_id_aa64pfr0_el1().is_feat_mpam_present()
}
