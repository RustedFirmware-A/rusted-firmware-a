// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_FGT2 CPU extension.

#[cfg(any(feature = "sel2", feature = "rme"))]
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld},
    platform::exception_free,
};
use crate::{
    context::{PerWorldContext, World},
    cpu_extensions::CpuExtension,
    platform::Platform,
};
#[cfg(any(feature = "sel2", feature = "rme"))]
use arm_sysregs::{
    Hdfgrtr2El2, Hdfgwtr2El2, Hfgitr2El2, Hfgrtr2El2, Hfgwtr2El2, read_hdfgrtr2_el2,
    read_hdfgwtr2_el2, read_hfgitr2_el2, read_hfgrtr2_el2, read_hfgwtr2_el2, write_hdfgrtr2_el2,
    write_hdfgwtr2_el2, write_hfgitr2_el2, write_hfgrtr2_el2, write_hfgwtr2_el2,
};
use arm_sysregs::{ScrEl3, read_id_aa64mmfr0_el1};
#[cfg(any(feature = "sel2", feature = "rme"))]
use core::cell::RefCell;
use core::marker::PhantomData;
#[cfg(any(feature = "sel2", feature = "rme"))]
use percore::{ExceptionLock, PerCore};

#[cfg(any(feature = "sel2", feature = "rme"))]
struct Fgt2CpuContext {
    hfgitr2_el2: Hfgitr2El2,
    hfgrtr2_el2: Hfgrtr2El2,
    hfgwtr2_el2: Hfgwtr2El2,
    hdfgrtr2_el2: Hdfgrtr2El2,
    hdfgwtr2_el2: Hdfgwtr2El2,
}

#[cfg(any(feature = "sel2", feature = "rme"))]
impl Fgt2CpuContext {
    const EMPTY: Self = Self {
        hfgitr2_el2: Hfgitr2El2::empty(),
        hfgrtr2_el2: Hfgrtr2El2::empty(),
        hfgwtr2_el2: Hfgwtr2El2::empty(),
        hdfgrtr2_el2: Hdfgrtr2El2::empty(),
        hdfgwtr2_el2: Hdfgwtr2El2::empty(),
    };
}

/// FEAT_FGT2 support
///
/// Enables support for the HFGITR2_EL2, HFGRTR2_EL2, HFGWTR_EL2, HDFGRTR2_EL2, and HDFGWTR2_EL2
/// registers. These are extensions of the corresponding FGT registers, allowing more control
/// control over the traps. They are saved and restored during world switches.
///
/// The extension is enabled for all worlds present in the system.
pub struct Fgt2<const CORE_COUNT: usize, PlatformImpl: Platform> {
    #[cfg(any(feature = "sel2", feature = "rme"))]
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<Fgt2CpuContext>>,
    _platform: PhantomData<PlatformImpl>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Fgt2<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the FGT2 CPU extension.
    pub const fn new() -> Self {
        Self {
            #[cfg(any(feature = "sel2", feature = "rme"))]
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [Fgt2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
            _platform: PhantomData,
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Fgt2<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Fgt2<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64mmfr0_el1().is_feat_fgt2_present()
    }

    fn configure_per_world(&self, _: World, context: &mut PerWorldContext) {
        context.scr_el3 |= ScrEl3::FGTEN2
    }

    #[cfg(any(feature = "sel2", feature = "rme"))]
    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                let mut ctx = self.context.get().borrow_mut(token);
                let ctx = &mut ctx[world];

                ctx.hfgitr2_el2 = read_hfgitr2_el2();
                ctx.hfgrtr2_el2 = read_hfgrtr2_el2();
                ctx.hfgwtr2_el2 = read_hfgwtr2_el2();
                ctx.hdfgrtr2_el2 = read_hdfgrtr2_el2();
                ctx.hdfgwtr2_el2 = read_hdfgwtr2_el2();
            })
        }
    }

    #[cfg(any(feature = "sel2", feature = "rme"))]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                let ctx = self.context.get().borrow_mut(token);
                let ctx = &ctx[world];

                // SAFETY: We're restoring the values previously saved, so they must be valid.
                unsafe {
                    write_hfgitr2_el2(ctx.hfgitr2_el2);
                    write_hfgrtr2_el2(ctx.hfgrtr2_el2);
                    write_hfgwtr2_el2(ctx.hfgwtr2_el2);
                    write_hdfgrtr2_el2(ctx.hdfgrtr2_el2);
                    write_hdfgwtr2_el2(ctx.hdfgwtr2_el2);
                }
            })
        }
    }
}
