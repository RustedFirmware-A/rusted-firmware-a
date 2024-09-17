// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    aarch64::{dsb_sy, isb},
    sysregs::{
        read_scr_el3, write_icc_sre_el1, write_icc_sre_el2, write_icc_sre_el3, write_scr_el3,
        IccSre, ScrEl3,
    },
};
use arm_gic::{
    gicv3::{registers::GicdCtlr, GICRError, GicV3, Group, SecureIntGroup},
    IntId, Trigger,
};

const GIC_HIGHEST_NS_PRIORITY: u8 = 0x80;
const GIC_PRI_MASK: u8 = 0xff;

/// The configuration of a single secure interrupt.
pub struct SecureInterruptConfig {
    /// ID of the configured interrupt.
    pub id: IntId,
    /// Interrupt priority.
    /// 0x00 is highest priority, 0xFF is the lowest.
    pub priority: u8,
    /// Secure interrupt group that this interrupt should belong to.
    pub group: SecureIntGroup,
    /// To specify whether this interrupt should be edge or level triggered.
    pub trigger: Trigger,
}

/// The configuration of platform's GIC.
pub struct GicConfig {
    /// This list specifies which interrupts will be configured
    /// to be secure interrupts.
    pub secure_interrupts_config: &'static [SecureInterruptConfig],
}

fn configure_defaults_for_spis(gic: &mut GicV3) {
    let num_spis = gic.typer().num_spis() as usize;

    for i in IntId::spis().take(num_spis) {
        // Treat all SPIs as G1NS by default.
        gic.set_group(i, None, Group::Group1NS);

        // Setup the default SPI priority
        gic.set_interrupt_priority(i, None, GIC_HIGHEST_NS_PRIORITY);

        // Treat all SPIs as level triggered by default.
        gic.set_trigger(i, None, Trigger::Level);
    }
}

fn configure_secure_spis(gic: &mut GicV3, config: &[SecureInterruptConfig]) {
    let mut flags: GicdCtlr = GicdCtlr::empty();

    for prop in config.iter().filter(|i| i.id.is_spi()) {
        gic.set_group(prop.id, None, Group::Secure(prop.group));

        // Set interrupt configuration.
        gic.set_trigger(prop.id, None, prop.trigger);

        // Set the priority of this interrupt.
        gic.set_interrupt_priority(prop.id, None, prop.priority);

        // Target (E)SPIs to the primary CPU.
        // TODO:
        // gic_affinity_val = gicd_irouter_val_from_mpidr(read_mpidr(), 0U);
        // gicd_write_irouter(multichip_gicd_base, intr_num, gic_affinity_val);

        // Enable this interrupt.
        gic.enable_interrupt(prop.id, None, true);

        match prop.group {
            SecureIntGroup::Group1S => flags |= GicdCtlr::EnableGrp1S,
            SecureIntGroup::Group0 => flags |= GicdCtlr::EnableGrp0,
        }
    }

    // Enable interrupt groups as required.
    gic.gicd_set_control(flags);
}

fn init_distributor(gic: &mut GicV3, config: &[SecureInterruptConfig]) {
    // Clear the "enable" bits for G0/G1S/G1NS interrupts before configuring
    // the ARE_S bit. The Distributor might generate a system error
    // otherwise.
    gic.gicd_clear_control(GicdCtlr::EnableGrp0 | GicdCtlr::EnableGrp1S | GicdCtlr::EnableGrp1NS);

    // Set the ARE_S and ARE_NS bits now that interrupts have been disabled.
    gic.gicd_set_control(GicdCtlr::ARE_S | GicdCtlr::ARE_NS);

    // Set the default attribute of all (E)SPIs.
    configure_defaults_for_spis(gic);

    configure_secure_spis(gic, config)
}

fn configure_defaults_for_private_interrupts(gic: &mut GicV3, core_index: usize) {
    // Assume we are running without GIC_EXT_INTID
    // So only 16 SGIs and 16 PPIs

    // Disable all SGIs (implementation defined)/PPIs before configuring them.
    for intid in IntId::private() {
        gic.enable_interrupt(intid, Some(core_index), false);
    }

    // Wait for pending writes
    gic.gicr_barrier(core_index);

    for intid in IntId::private() {
        // Treat all SGIs/PPIs as G1NS by default
        gic.set_group(intid, Some(core_index), Group::Group1NS);

        // Set default priority for all SGIs and PPIs.
        gic.set_interrupt_priority(intid, Some(core_index), GIC_HIGHEST_NS_PRIORITY);

        // Make all SGIs and PPIs level triggered.
        gic.set_trigger(intid, Some(core_index), Trigger::Level);
    }
}

fn configure_secure_private_interrupts(
    gic: &mut GicV3,
    core_index: usize,
    config: &[SecureInterruptConfig],
) {
    let mut flags: GicdCtlr = GicdCtlr::empty();

    for prop in config.iter().filter(|i| i.id.is_private()) {
        gic.set_group(prop.id, Some(core_index), Group::Secure(prop.group));

        gic.set_interrupt_priority(prop.id, Some(core_index), prop.priority);

        // Set interrupt configuration for PPIs.
        // Configurations for SGIs 0-15 are ignored.
        if prop.id.is_ppi() {
            gic.set_trigger(prop.id, Some(core_index), prop.trigger);
        }

        gic.enable_interrupt(prop.id, Some(core_index), true);

        match prop.group {
            SecureIntGroup::Group1S => flags |= GicdCtlr::EnableGrp1S,
            SecureIntGroup::Group0 => flags |= GicdCtlr::EnableGrp0,
        }
    }

    // Enable interrupt groups as required.
    gic.gicd_set_control(flags);
}

fn init_redistributor(gic: &mut GicV3, core_index: usize, config: &[SecureInterruptConfig]) {
    // TODO: power on redistributor for GIC-600

    // TODO: Find the redistributor base address for given CPU.
    configure_defaults_for_private_interrupts(gic, core_index);

    configure_secure_private_interrupts(gic, core_index, config);
}

fn init_cpu_interface(gic: &mut GicV3, core_index: usize) -> Result<(), GICRError> {
    gic.redistributor_mark_core_awake(core_index)?;

    // Disable the legacy interrupt bypass
    // Enable system register access for EL3 and allow lower exception
    // levels to configure the same for themselves. If the legacy mode is
    // not supported, the SRE bit is RAO/WI
    let icc_sre_el3 = IccSre::DIB | IccSre::DFB | IccSre::EN | IccSre::SRE;

    // SAFETY: This is the only place we set `icc_sre_el3`, and we set the SRE bit, so it is never
    // changed from 1 to 0.
    unsafe { write_icc_sre_el3(icc_sre_el3) };

    let scr_el3 = read_scr_el3();

    // Switch to NS state to write Non secure ICC_SRE_EL1 and
    // ICC_SRE_EL2 registers.
    write_scr_el3(scr_el3 | ScrEl3::NS);
    isb();

    // TODO: if the sel2 feature is enabled then icc_sre_el2 will be overwritten
    // when the El2Sysregs of the first world to start are restored,
    // so we are not sure that this even has any effect.
    write_icc_sre_el2(icc_sre_el3);
    write_icc_sre_el1(IccSre::SRE);
    isb();

    // Switch to secure state.
    write_scr_el3(scr_el3 - ScrEl3::NS);
    isb();

    // Write the secure ICC_SRE_EL1 register.
    write_icc_sre_el1(IccSre::SRE);
    isb();

    // Program the idle priority in the PMR.
    GicV3::set_priority_mask(GIC_PRI_MASK);

    // Enable Group0 interrupts.
    GicV3::enable_group0(true);

    // Enable Group1 Secure interrupts.
    GicV3::enable_group1(true);

    // Restore the original state.
    write_scr_el3(scr_el3);

    isb();
    dsb_sy();
    Ok(())
}

pub fn init(gic: &mut GicV3, config: &GicConfig, core_index: usize) {
    init_distributor(gic, config.secure_interrupts_config);
    init_redistributor(gic, core_index, config.secure_interrupts_config);
    // TODO: Handle the error.
    init_cpu_interface(gic, core_index).unwrap();
}
