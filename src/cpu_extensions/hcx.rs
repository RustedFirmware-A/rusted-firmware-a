// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! The Extended Hypervisor Configuration CPU extension.

#[cfg(feature = "sel2")]
mod hcx_sel2;

#[cfg(feature = "sel2")]
use self::hcx_sel2::HcxCpuContext;
use super::CpuExtension;
#[cfg(feature = "sel2")]
use crate::context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld};
use crate::{
    context::{PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{HcrxEl2, ScrEl3, read_id_aa64mmfr1_el1, write_hcrx_el2};
#[cfg(feature = "sel2")]
use core::cell::RefCell;
use core::marker::PhantomData;
#[cfg(feature = "sel2")]
use percore::{ExceptionLock, PerCore};

/// FEAT_HCX introduces the Extended Hypervisor Configuration Register, HCRX_EL2, that provides
/// configuration controls for virtualization in addition to those provided by HCR_EL2, including
/// defining whether various operations are trapped to EL2.
pub struct Hcx<const CORE_COUNT: usize, PlatformImpl: Platform> {
    #[cfg(feature = "sel2")]
    context: PerCoreState<{ CORE_COUNT }, PlatformImpl, PerWorld<HcxCpuContext>>,
    _platform: PhantomData<PlatformImpl>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Hcx<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the HCX CPU extension.
    pub const fn new() -> Self {
        Self {
            #[cfg(feature = "sel2")]
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [HcxCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
            _platform: PhantomData,
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Hcx<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Hcx<CORE_COUNT, PlatformImpl>
{
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
