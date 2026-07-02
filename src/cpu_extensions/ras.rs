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
use crate::context::{CPU_DATA_CONTEXT_NUM, PerWorld, World};
use core::cell::RefCell;
use percore::{ExceptionLock, derive::percore};

#[percore]
static RAS_CONTEXT: ExceptionLock<RefCell<PerWorld<RasCpuContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([RasCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// Enables context switching of the Reliability, Accessibility, Serviceability (RAS) extension
/// registers on world switch. If RAS features are used by lower ELs then this extension must be
/// enabled.
pub struct Ras;

impl CpuExtension for Ras {
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
