// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Statistical Profiling Extension

use super::CpuExtension;
use crate::context::{CpuContext, World};
use arm_sysregs::{MdcrEl3, read_id_aa64dfr0_el1};

/// Statistical Profiling Extension
///
/// Configures the Statistical Profiling Extension (FEAT_SPE) so that the Non-secure world owns the
/// profiling buffer, and profiling is disabled in both the Secure and Realm worlds.
///
/// FEAT_SPE provides a non-invasive method of sampling software and hardware using randomized
/// sampling of either architectural instructions, as defined by the instruction set architecture,
/// or by microarchitectural operations.
pub struct StatisticalProfiling;

impl CpuExtension for StatisticalProfiling {
    fn is_present(&self) -> bool {
        read_id_aa64dfr0_el1().is_feat_spe_present()
    }

    fn configure_per_cpu(&self, world: World, context: &mut CpuContext) {
        if world == World::NonSecure {
            // MDCR_EL3.NSPB (ARM v8.2): SPE enabled in Non-secure state and disabled in secure
            // state. Accesses to SPE registers at S-EL1 generate trap exceptions to EL3.
            //
            // MDCR_EL3.NSPBE: Profiling Buffer uses Non-secure Virtual Addresses. When FEAT_RME is
            // not implemented, this field is RES0.
            //
            // MDCR_EL3.EnPMSN (ARM v8.7) and MDCR_EL3.EnPMS3: Do not trap access to PMSNEVFR_EL1 or
            // PMSDSFR_EL1 register at NS-EL1 or NS-EL2 to EL3 if FEAT_SPEv1p2 or FEAT_SPE_FDS are
            // implemented. Setting these bits to 1 doesn't have any effect on it when the features
            // aren't implemented.
            context.el3_state.mdcr_el3 |= MdcrEl3::NSPB_NS | MdcrEl3::ENPMSN | MdcrEl3::ENPMS3;
            context.el3_state.mdcr_el3 -= MdcrEl3::NSPBE;
        }
    }
}
