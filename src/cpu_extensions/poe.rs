// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Stage 1/2 permission overlays

#[cfg(not(feature = "sel2"))]
mod s1poe_sel1;
#[cfg(feature = "sel2")]
mod s1poe_sel2;
#[cfg(not(feature = "sel2"))]
mod s2poe_sel1;

#[cfg(not(feature = "sel2"))]
use self::s1poe_sel1::S1PoeContextEl1 as S1PoeContext;
#[cfg(feature = "sel2")]
use self::s1poe_sel2::S1PoeContextEl2 as S1PoeContext;
#[cfg(not(feature = "sel2"))]
use self::s2poe_sel1::S2PoeContext;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerWorld, PerWorldContext, World},
    cpu_extensions::CpuExtension,
    platform::exception_free,
};
use arm_sysregs::{el1::accessors::read_id_aa64mmfr3_el1, el3::registers::ScrEl3};
use core::cell::RefCell;
use percore::{ExceptionLock, derive::percore};

#[percore]
static S1POE_CONTEXT: ExceptionLock<RefCell<PerWorld<S1PoeContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([S1PoeContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

#[cfg(not(feature = "sel2"))]
#[percore]
static S2POE_CONTEXT: ExceptionLock<RefCell<PerWorld<S2PoeContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([S2PoeContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// `FEAT_S1POE`, Stage 1 permission overlay extension
///
/// Disables trapping for lower EL access of the extension's system register and handles context save
/// on world switch.
pub struct S1Poe;

impl CpuExtension for S1Poe {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_s1poe_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::PIEN;
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S1POE_CONTEXT.get().borrow_mut(token)[world].save();
            });
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S1POE_CONTEXT.get().borrow(token).borrow()[world].restore();
            });
        }
    }
}

/// `FEAT_S2POE`, Stage 2 permission overlay extension
///
/// Disables trapping for lower EL access of the extension's system register and handles context
/// save on world switch if EL1 context save is needed.
pub struct S2Poe;

impl CpuExtension for S2Poe {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_s2poe_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::PIEN;
    }

    #[cfg(not(feature = "sel2"))]
    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S2POE_CONTEXT.get().borrow_mut(token)[world].save();
            });
        }
    }

    #[cfg(not(feature = "sel2"))]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S2POE_CONTEXT.get().borrow(token).borrow()[world].restore();
            });
        }
    }
}
