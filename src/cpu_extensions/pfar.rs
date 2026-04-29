// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Physical Fault Address Register Extension

#[cfg(not(feature = "sel2"))]
mod pfar_sel1;
#[cfg(feature = "sel2")]
mod pfar_sel2;

#[cfg(not(feature = "sel2"))]
use self::pfar_sel1::PfarContext;
#[cfg(feature = "sel2")]
use self::pfar_sel2::PfarContext;
use super::CpuExtension;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{ScrEl3, read_id_aa64pfr1_el1};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

/// FEAT_PFAR support
///
/// Configures the Physical Fault Address Register Extension (FEAT_PFAR)
/// so that lower exception levels can access it.
///
/// Allows PFAR_EL1 and PFAR_EL2 to be used.
///
/// PFAR_ELx records the faulting physical address for a synchronous External abort or SError
/// exception.
pub struct Pfar<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<PfarContext>>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Pfar<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the PFAR CPU extension.
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [PfarContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Pfar<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Pfar<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64pfr1_el1().is_feat_pfar_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::PFAREN;
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
