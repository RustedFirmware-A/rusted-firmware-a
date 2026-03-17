// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Memory Tagging Extension

#[cfg(not(feature = "sel2"))]
mod mte2_sel1;
#[cfg(feature = "sel2")]
mod mte2_sel2;

use super::CpuExtension;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{ScrEl3, read_id_aa64pfr1_el1};
use core::cell::RefCell;
#[cfg(not(feature = "sel2"))]
use mte2_sel1::Mte2CpuContext;
#[cfg(feature = "sel2")]
use mte2_sel2::Mte2CpuContext;
use percore::{ExceptionLock, PerCore};

/// Memory Tagging Extension
///
/// Configures the Memory Tagging Extension (FEAT_MTE2) to enable the Non-secure and Secure worlds
/// to use it.
///
/// FEAT_MTE2 provides architectural support for runtime, always-on detection of various classes of
/// memory error to aid with software debugging to eliminate vulnerabilities arising from
/// memory-unsafe languages.
pub struct MemoryTagging<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<Mte2CpuContext>>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> MemoryTagging<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the MTE CPU extension.
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [Mte2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default
    for MemoryTagging<CORE_COUNT, PlatformImpl>
{
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for MemoryTagging<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        mte2_is_present()
    }

    fn configure_per_world(&self, world: World, context: &mut PerWorldContext) {
        // Allow access to Allocation Tags for Non-secure and Secure worlds when FEAT_MTE2 is
        // implemented.
        if world == World::NonSecure || world == World::Secure {
            context.scr_el3 |= ScrEl3::ATA;
        }
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            self.save_context_internal(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            self.restore_context_internal(world);
        }
    }
}

/// Returns whether MTE2 is supported on the system.
pub fn mte2_is_present() -> bool {
    read_id_aa64pfr1_el1().is_feat_mte2_present()
}
