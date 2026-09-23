// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Cache Speculation Variant 2 Version 2 (FEAT_CSV2_2) Extension

#[cfg(not(feature = "sel2"))]
mod csv2_2_sel1;
#[cfg(feature = "sel2")]
mod csv2_2_sel2;

#[cfg(not(feature = "sel2"))]
use self::csv2_2_sel1::Csv2_2ContextSel1 as Csv2_2Context;
#[cfg(feature = "sel2")]
use self::csv2_2_sel2::Csv2_2ContextSel2 as Csv2_2Context;
use super::CpuExtension;
use crate::context::{CPU_DATA_CONTEXT_NUM, PerWorld, PerWorldContext, World};
use arm_sysregs::{el1::accessors::read_id_aa64pfr0_el1, el3::registers::ScrEl3};
use core::cell::RefCell;
use percore::{ExceptionLock, derive::percore};

#[percore]
static CSV2_2CONTEXT: ExceptionLock<RefCell<PerWorld<Csv2_2Context>>> = ExceptionLock::new(
    RefCell::new(PerWorld([Csv2_2Context::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// FEAT_CSV2_2 support
///
/// Configures the Cache Speculation Variant 2 Version 2 Extension (FEAT_CSV2_2)
/// so that lower exception levels can access it.
///
/// Allows SCXTNUM_EL0, SCXTNUM_EL1, and SCXTNUM_EL2 to be used.
///
/// When FEAT_CSV2_2 is implemented, the SCXTNUM_ELx registers are part of the
/// hardware-defined context used by FEAT_CSV2. FEAT_CSV2 disallows code running in
/// one context to exploitatively control, or predictively leak to code running in
/// a different context for a number of scenarios.
pub struct Csv2_2;

impl CpuExtension for Csv2_2 {
    fn is_present(&self) -> bool {
        read_id_aa64pfr0_el1().is_feat_csv2_2_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::ENSCXT;
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
