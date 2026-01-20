// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 introduces the TCR2_ELx registers which provide top-level control of the EL1&0
//! and EL2&0 translation regimes respectively.

#[cfg(not(feature = "sel2"))]
mod tcr2_sel1;
#[cfg(feature = "sel2")]
mod tcr2_sel2;

#[cfg(not(feature = "sel2"))]
use self::tcr2_sel1::Tcr2CpuContext;
#[cfg(feature = "sel2")]
use self::tcr2_sel2::Tcr2CpuContext;
use super::CpuExtension;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{ScrEl3, read_id_aa64mmfr3_el1};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

/// Enables access to the TCR2_ELx registers at lower ELs, along with context switching of those
/// registers on world switch.
pub struct Tcr2<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<Tcr2CpuContext>>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Tcr2<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the TCR2 CPU extension.
    #[allow(dead_code)]
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [Tcr2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Tcr2<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Tcr2<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_tcr2_present()
    }

    fn configure_per_world(&self, _world: World, context: &mut PerWorldContext) {
        // Enable access to TCR2_ELx registers at lower ELs.
        context.scr_el3 |= ScrEl3::TCR2EN;
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            self.save_registers(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            self.restore_registers(world);
        }
    }
}
