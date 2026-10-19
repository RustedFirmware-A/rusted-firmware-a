// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Stage 1/2 permission indirections extensions (`FEAT_S1PIE` and `FEAT_S2PIE`)

#[cfg(not(feature = "sel2"))]
mod s1pie_sel1;
#[cfg(feature = "sel2")]
mod s1pie_sel2;
#[cfg(feature = "sel2")]
mod s2pie_sel2;

#[cfg(not(feature = "sel2"))]
use self::s1pie_sel1::S1PieContextEl1 as S1PieContext;
#[cfg(feature = "sel2")]
use self::{s1pie_sel2::S1PieContextEl2 as S1PieContext, s2pie_sel2::S2PieContext};
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerWorld, PerWorldContext, World},
    cpu_extensions::CpuExtension,
    platform::exception_free,
};
use arm_sysregs::{el1::accessors::read_id_aa64mmfr3_el1, el3::registers::ScrEl3};
use core::cell::RefCell;
use percore::{ExceptionLock, derive::percore};

#[percore]
static S1PIE_CONTEXT: ExceptionLock<RefCell<PerWorld<S1PieContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([S1PieContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

#[cfg(feature = "sel2")]
#[percore]
static S2PIE_CONTEXT: ExceptionLock<RefCell<PerWorld<S2PieContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([S2PieContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// FEAT_S1PIE, Stage 1 permission indirections extension
///
/// Disables trapping for lower EL access of the extension's system register and handles context
/// save on world switch.
pub struct S1Pie;

impl CpuExtension for S1Pie {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_s1pie_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::PIEN;
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S1PIE_CONTEXT.get().borrow_mut(token)[world].save();
            });
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S1PIE_CONTEXT.get().borrow(token).borrow()[world].restore();
            });
        }
    }
}

/// FEAT_S2PIE, Stage 2 permission indirections extension
///
/// Disables trapping for lower EL access of the extension's system register and handles context
/// save on world switch if EL2 context save is needed.
pub struct S2Pie;

impl CpuExtension for S2Pie {
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_s2pie_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::PIEN;
    }

    #[cfg(feature = "sel2")]
    fn save_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S2PIE_CONTEXT.get().borrow_mut(token)[world].save();
            });
        }
    }

    #[cfg(feature = "sel2")]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            exception_free(|token| {
                S2PIE_CONTEXT.get().borrow(token).borrow()[world].restore();
            });
        }
    }
}
