// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::panic;

use crate::{
    aarch64::{dsb_sy, isb},
    context::CoresImpl,
    platform::{Platform, PlatformImpl},
    sysregs::{write_icc_sre_el3, IccSre},
};

use arm_gic::{
    gicv3::{registers::GicdCtlr, GICRError, GicV3, Group},
    IntId, Trigger,
};
use percore::Cores;
use spin::{mutex::SpinMutex, Once};

static GIC: Once<SpinMutex<GicV3>> = Once::new();

const GIC_HIGHEST_NS_PRIORITY: u8 = 0x80;
const GIC_PRI_MASK: u8 = 0xff;

/// The configuration of a single interrupt.
#[derive(Clone, Copy, Debug)]
pub struct InterruptConfig {
    /// Interrupt priority.
    /// 0x00 is highest priority, 0xFF is the lowest.
    pub priority: u8,
    /// Interrupt group that this interrupt should belong to.
    pub group: Group,
    /// To specify whether this interrupt should be edge or level triggered.
    pub trigger: Trigger,
}

impl Default for InterruptConfig {
    fn default() -> Self {
        Self {
            priority: GIC_HIGHEST_NS_PRIORITY,
            group: Group::Group1NS,
            trigger: Trigger::Level,
        }
    }
}

pub type InterruptConfigEntry = (IntId, InterruptConfig);

/// The configuration of platform's GIC.
pub struct GicConfig {
    /// This list specifies which interrupts will be configured
    /// to non-default setup.
    pub interrupts_config: &'static [InterruptConfigEntry],
}

impl GicConfig {
    fn get_interrupt_config_or_default(&self, int_id: IntId) -> InterruptConfig {
        self.interrupts_config
            .iter()
            .find(|(id, _)| *id == int_id)
            .map(|(_, cfg)| *cfg)
            .unwrap_or_default()
    }
}

/// Configure all available Shared Peripheral Interrupts (SPIs).
fn configure_spis(gic: &mut GicV3, config: &GicConfig) {
    let num_spis = gic.typer().num_spis() as usize;

    // Disable all SPIs before configuring them.
    for int_id in IntId::spis().take(num_spis) {
        gic.enable_interrupt(int_id, None, false);
    }

    gic.gicd_barrier();

    for int_id in IntId::spis().take(num_spis) {
        let cfg = config.get_interrupt_config_or_default(int_id);

        gic.set_group(int_id, None, cfg.group);
        gic.set_trigger(int_id, None, cfg.trigger);
        gic.set_interrupt_priority(int_id, None, cfg.priority);

        // Target (E)SPIs to the primary CPU.
        // TODO:
        // gic_affinity_val = gicd_irouter_val_from_mpidr(read_mpidr(), 0U);
        // gicd_write_irouter(multichip_gicd_base, intr_num, gic_affinity_val);
    }

    // Enable all SPIs.
    for int_id in IntId::spis().take(num_spis) {
        gic.enable_interrupt(int_id, None, true);
    }
}

fn init_distributor(gic: &mut GicV3, config: &GicConfig) {
    // Clear the "enable" bits for G0/G1S/G1NS interrupts before configuring
    // the ARE_S bit. The Distributor might generate a system error
    // otherwise.
    gic.gicd_clear_control(GicdCtlr::EnableGrp0 | GicdCtlr::EnableGrp1S | GicdCtlr::EnableGrp1NS);

    // Set the ARE_S and ARE_NS bits now that interrupts have been disabled.
    gic.gicd_set_control(GicdCtlr::ARE_S | GicdCtlr::ARE_NS);

    configure_spis(gic, config);

    // Enable all interrupt groups back.
    gic.gicd_set_control(GicdCtlr::EnableGrp0 | GicdCtlr::EnableGrp1S | GicdCtlr::EnableGrp1NS);
}

/// Configure all available SGIs and PPIs.
fn configure_private_interrupts(gic: &mut GicV3, core_index: usize, config: &GicConfig) {
    // Assume we are running without GIC_EXT_INTID
    // So only 16 SGIs and 16 PPIs

    // Disable all private interrupts before configuring them.
    for int_id in IntId::private() {
        gic.enable_interrupt(int_id, Some(core_index), false);
    }

    // Wait for pending writes
    gic.gicr_barrier(core_index);

    for int_id in IntId::private() {
        let cfg = config.get_interrupt_config_or_default(int_id);

        gic.set_group(int_id, Some(core_index), cfg.group);
        gic.set_interrupt_priority(int_id, Some(core_index), cfg.priority);

        // Set interrupt configuration for PPIs.
        // Configurations for SGIs 0-15 are ignored.
        if int_id.is_ppi() {
            gic.set_trigger(int_id, Some(core_index), cfg.trigger);
        }
    }

    // Enable all private interrupts.
    for int_id in IntId::private() {
        gic.enable_interrupt(int_id, Some(core_index), true);
    }
}

fn init_redistributor(gic: &mut GicV3, core_index: usize, config: &GicConfig) {
    // TODO: power on redistributor for GIC-600
    configure_private_interrupts(gic, core_index, config);
}

fn init_cpu_interface(gic: &mut GicV3, core_index: usize) -> Result<(), GICRError> {
    gic.redistributor_mark_core_awake(core_index)?;

    // Disable the legacy interrupt bypass.
    // Enable system register access for EL3 and allow lower exception
    // levels to configure the same for themselves. If the legacy mode is
    // not supported, the SRE bit is RAO/WI.
    let icc_sre = IccSre::DIB | IccSre::DFB | IccSre::EN | IccSre::SRE;

    // SAFETY: This is the only place we set `icc_sre_el3`, and we set the SRE bit, so it is never
    // changed from 1 to 0.
    unsafe { write_icc_sre_el3(icc_sre) };

    // Program the idle priority in the PMR.
    GicV3::set_priority_mask(GIC_PRI_MASK);

    // Enable Group0 interrupts.
    GicV3::enable_group0(true);

    // Enable Group1 Secure interrupts.
    GicV3::enable_group1(true);

    isb();
    dsb_sy();
    Ok(())
}

/// Initializes the gic by configuring the distributor, redistributor and cpu interface, and puts
/// the global gic into GIC variable. This function should only be called once early in the boot
/// process. Subsequent calls will be ignored.
pub fn init() {
    GIC.call_once(|| {
        // SAFETY: This is the only place where GIC is created and there are no aliases.
        let mut gic = unsafe { PlatformImpl::create_gic() };

        init_distributor(&mut gic, &PlatformImpl::GIC_CONFIG);

        // Configure Redistributors for all cores.
        // Secondary cores must configure only their CPU interfaces.
        for core_idx in 0..PlatformImpl::CORE_COUNT {
            init_redistributor(&mut gic, core_idx, &PlatformImpl::GIC_CONFIG);
        }

        // thiserror does not print the error message with `expect` :(
        if let Err(e) = init_cpu_interface(&mut gic, CoresImpl::core_index()) {
            panic!("Failed to init GIC: {}", e);
        }

        SpinMutex::new(gic)
    });
}
