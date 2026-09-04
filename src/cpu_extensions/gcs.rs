// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Guarded Control Stack Extension

#[cfg(not(feature = "sel2"))]
mod gcs_sel1;
#[cfg(feature = "sel2")]
mod gcs_sel2;

#[cfg(not(feature = "sel2"))]
use self::gcs_sel1::GcsContextEl1 as GcsContext;
#[cfg(feature = "sel2")]
use self::gcs_sel2::GcsContextEl2 as GcsContext;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerWorld, PerWorldContext, World},
    cpu_extensions::CpuExtension,
};
use arm_sysregs::{el1::accessors::read_id_aa64pfr1_el1, el3::registers::ScrEl3};
use core::cell::RefCell;
use percore::{ExceptionLock, derive::percore};

#[percore]
static GCS_CONTEXT: ExceptionLock<RefCell<PerWorld<GcsContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([GcsContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// FEAT_GCS support
///
/// Configures the Guarded Control Stack Extension (FEAT_GCS)
/// so that lower exception levels can access it.
pub struct Gcs;

impl CpuExtension for Gcs {
    fn is_present(&self) -> bool {
        read_id_aa64pfr1_el1().is_feat_gcs_present()
    }

    fn configure_per_world(&self, _world: World, ctx: &mut PerWorldContext) {
        ctx.scr_el3 |= ScrEl3::GCSEN;
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
