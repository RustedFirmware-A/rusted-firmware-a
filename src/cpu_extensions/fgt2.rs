// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::cell::RefCell;

use arm_sysregs::{
    ScrEl3, read_hdfgrtr2_el2, read_hdfgwtr2_el2, read_hfgitr2_el2, read_hfgrtr2_el2,
    read_hfgwtr_el2, read_id_aa64mmfr0_el1, write_hdfgrtr2_el2, write_hdfgwtr2_el2,
    write_hfgitr2_el2, write_hfgrtr2_el2, write_hfgwtr_el2,
};
use percore::{ExceptionLock, PerCore};

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, CpuContext, PerCoreState, PerWorld, World},
    cpu_extensions::CpuExtension,
    platform::{Platform, PlatformImpl, exception_free},
};

struct Fgt2CpuContext {
    hfgitr2_el2: u64,
    hfgrtr2_el2: u64,
    hfgwtr_el2: u64,
    hdfgrtr2_el2: u64,
    hdfgwtr2_el2: u64,
}

impl Fgt2CpuContext {
    const EMPTY: Self = Self {
        hfgitr2_el2: 0,
        hfgrtr2_el2: 0,
        hfgwtr_el2: 0,
        hdfgrtr2_el2: 0,
        hdfgwtr2_el2: 0,
    };
}

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
pub struct Fgt2;

impl CpuExtension for Fgt2 {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr0_el1().is_feat_fgt2_present()
    }

    fn configure_per_cpu(&self, _world: World, context: &mut CpuContext) {
        context.el3_state.scr_el3 |= ScrEl3::FGTEN2
    }

    #[allow(unused)]
    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                let mut ctx = FGT2_CTX.get().borrow_mut(token);
                let ctx = &mut ctx[world];

                ctx.hfgitr2_el2 = read_hfgitr2_el2();
                ctx.hfgrtr2_el2 = read_hfgrtr2_el2();
                ctx.hfgwtr_el2 = read_hfgwtr_el2();
                ctx.hdfgrtr2_el2 = read_hdfgrtr2_el2();
                ctx.hdfgwtr2_el2 = read_hdfgwtr2_el2();
            })
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                let ctx = FGT2_CTX.get().borrow_mut(token);
                let ctx = &ctx[world];

                write_hfgitr2_el2(ctx.hfgitr2_el2);
                write_hfgrtr2_el2(ctx.hfgrtr2_el2);
                write_hfgwtr_el2(ctx.hfgwtr_el2);
                write_hdfgrtr2_el2(ctx.hdfgrtr2_el2);
                write_hdfgwtr2_el2(ctx.hdfgwtr2_el2);
            })
        }
    }
}
