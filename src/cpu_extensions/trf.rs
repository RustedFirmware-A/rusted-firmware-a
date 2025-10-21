// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Controls lower EL access to Trace filter control registers.

use super::CpuExtension;

use crate::context::{CpuContext, World};

use arm_sysregs::{MdcrEl3, read_id_aa64dfr0_el1};

/// Enables lower EL access to Trace Filter control registers.
pub struct TraceFiltering;

impl CpuExtension for TraceFiltering {
    fn is_present(&self) -> bool {
        read_id_aa64dfr0_el1().is_feat_trf_present()
    }

    fn configure_per_cpu(&self, _world: World, ctx: &mut CpuContext) {
        // Allow access of trace filter control registers for lower ELs.
        //
        // Trace is by default prohibited in Secure and Realm states unless overridden by the
        // IMPLEMENTATION DEFINED authentication interface.
        ctx.el3_state.mdcr_el3 -= MdcrEl3::TTRF;
    }
}
