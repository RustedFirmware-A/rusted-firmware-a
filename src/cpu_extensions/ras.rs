// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Reliability, Accessibility, Serviceability (RAS) extension.

#[cfg(not(feature = "sel2"))]
mod ras_sel1;
#[cfg(feature = "sel2")]
mod ras_sel2;

#[cfg(not(feature = "sel2"))]
use self::ras_sel1::RasCpuContext;
#[cfg(feature = "sel2")]
use self::ras_sel2::RasCpuContext;
use super::CpuExtension;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::Platform,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

/// Enables context switching of the Reliability, Accessibility, Serviceability (RAS) extension
/// registers on world switch. If RAS features are used by lower ELs then this extension must be
/// enabled.
pub struct Ras<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<RasCpuContext>>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Ras<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the RAS CPU extension.
    #[allow(dead_code)]
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [RasCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Ras<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Ras<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        /* Assume that FEAT_RAS is present as it is mandatory from Armv8.2 */
        true
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
