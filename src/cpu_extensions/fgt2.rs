// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::cell::RefCell;

use arm_sysregs::{
    Hdfgrtr2El2, Hdfgwtr2El2, Hfgitr2El2, Hfgrtr2El2, Hfgwtr2El2, ScrEl3, read_id_aa64mmfr0_el1,
};
#[cfg(any(feature = "sel2", feature = "rme"))]
use arm_sysregs::{
    read_hdfgrtr2_el2, read_hdfgwtr2_el2, read_hfgitr2_el2, read_hfgrtr2_el2, read_hfgwtr2_el2,
    write_hdfgrtr2_el2, write_hdfgwtr2_el2, write_hfgitr2_el2, write_hfgrtr2_el2,
    write_hfgwtr2_el2,
};
use percore::{ExceptionLock, PerCore};

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, PerWorldContext, World},
    cpu_extensions::CpuExtension,
    platform::{Platform, PlatformImpl},
};

#[cfg(any(feature = "sel2", feature = "rme"))]
use crate::platform::exception_free;

#[allow(dead_code)]
struct Fgt2CpuContext {
    hfgitr2_el2: Hfgitr2El2,
    hfgrtr2_el2: Hfgrtr2El2,
    hfgwtr2_el2: Hfgwtr2El2,
    hdfgrtr2_el2: Hdfgrtr2El2,
    hdfgwtr2_el2: Hdfgwtr2El2,
}

impl Fgt2CpuContext {
    const EMPTY: Self = Self {
        hfgitr2_el2: Hfgitr2El2::empty(),
        hfgrtr2_el2: Hfgrtr2El2::empty(),
        hfgwtr2_el2: Hfgwtr2El2::empty(),
        hdfgrtr2_el2: Hdfgrtr2El2::empty(),
        hdfgwtr2_el2: Hdfgwtr2El2::empty(),
    };
}

#[allow(dead_code)]
static FGT2_CTX: PerCoreState<PerWorld<Fgt2CpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [Fgt2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

/// FEAT_FGT2 support
///
/// Enables support for the HFGITR2_EL2, HFGRTR2_EL2, HFGWTR_EL2, HDFGRTR2_EL2, and HDFGWTR2_EL2
/// registers. These are extensions of the corresponding FGT registers, allowing more control
/// control over the traps. They are saved and restored during world switches.
///
/// The extension is enabled for all worlds present in the system.
#[allow(dead_code)]
pub struct Fgt2;

impl CpuExtension for Fgt2 {
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
                let mut ctx = FGT2_CTX.get().borrow_mut(token);
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
                let ctx = FGT2_CTX.get().borrow_mut(token);
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
