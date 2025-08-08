// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::panic;

use crate::{
    aarch64::{dsb_sy, isb},
    context::{CoresImpl, World},
    platform::{Platform, PlatformImpl, plat_calc_core_pos},
    sysregs::{IccSre, MpidrEl1, ScrEl3, write_icc_sre_el3},
};
use arm_gic::{
    IntId, Trigger,
    gicv3::{GICRError, GicV3, Group, InterruptGroup, registers::GicdCtlr},
};
use log::debug;
use percore::Cores;
use spin::{Once, mutex::SpinMutex};

const GIC_HIGHEST_NS_PRIORITY: u8 = 0x80;
const GIC_PRI_MASK: u8 = 0xff;

pub static GIC: Once<Gic> = Once::new();

pub struct Gic<'a> {
    pub gic: SpinMutex<GicV3<'a>>,
    /// Redistributor indices by core ID.
    redistributor_indices: [usize; PlatformImpl::CORE_COUNT],
}

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

/// Specifies where an interrupt should be handled.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InterruptType {
    El3,
    Secure,
    NonSecure,
    Invalid,
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
    // TODO: power off the redistributor in PSCI ops.
    gic.gicr_power_on(core_index);

    configure_private_interrupts(gic, core_index, config);
}

fn init_cpu_interface(gic: &mut GicV3) -> Result<(), GICRError> {
    gic.redistributor_mark_core_awake(current_redistributor_index())?;

    // Disable the legacy interrupt bypass.
    // Enable system register access for EL3 and allow lower exception
    // levels to configure the same for themselves. If the legacy mode is
    // not supported, the SRE bit is RAO/WI.
    let icc_sre = IccSre::DIB | IccSre::DFB | IccSre::EN | IccSre::SRE;

    // SAFETY: This is the only place we set the SRE bit of `icc_sre_el3` to 1 and no other place
    // is permitted to change it from 1 to 0.
    unsafe { write_icc_sre_el3(icc_sre) };

    // Program the idle priority in the PMR.
    GicV3::set_priority_mask(GIC_PRI_MASK);

    GicV3::enable_group0(true);
    GicV3::enable_group1(true);

    isb();
    dsb_sy();
    Ok(())
}

/// Disables the GIC CPU interface of the calling CPU using system register accesses.
#[allow(unused)]
pub fn disable_cpu_interface(gic: &mut GicV3) -> Result<(), GICRError> {
    GicV3::enable_group0(false);
    GicV3::enable_group1(false);

    // Synchronize accesses to group enable registers.
    isb();

    // Ensure visibility of system register writes.
    dsb_sy();

    // TODO: enable errata wa 2384374

    gic.redistributor_mark_core_asleep(current_redistributor_index())?;
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
        for redistributor_index in 0..PlatformImpl::CORE_COUNT {
            init_redistributor(&mut gic, redistributor_index, &PlatformImpl::GIC_CONFIG);
        }

        // Calculate redistributor index for each CPU and store it for future use.
        let redistributor_indices = calculate_redistributor_indices(&mut gic);

        Gic {
            gic: SpinMutex::new(gic),
            redistributor_indices,
        }
    });

    // thiserror does not print the error message with `expect` :(
    if let Err(e) = init_cpu_interface(&mut GIC.get().unwrap().gic.lock()) {
        panic!("Failed to init GIC CPU interface: {}", e);
    }
    debug!(
        "GIC redistributor indices by core: {:?}",
        GIC.get().unwrap().redistributor_indices
    );
}

fn current_redistributor_index() -> usize {
    GIC.get().unwrap().redistributor_indices[CoresImpl::core_index()]
}

fn calculate_redistributor_indices(gic: &mut GicV3) -> [usize; PlatformImpl::CORE_COUNT] {
    let mut redistributor_indices = [0; PlatformImpl::CORE_COUNT];
    for redistributor_index in 0..PlatformImpl::CORE_COUNT {
        let mpidr = MpidrEl1::from_psci_mpidr(gic.gicr_typer(redistributor_index).core_mpidr());
        if !PlatformImpl::mpidr_is_valid(mpidr) {
            panic!("GIC redistributor {redistributor_index} has invalid mpidr value.");
        }
        let core_index = plat_calc_core_pos(mpidr.bits());
        redistributor_indices[core_index] = redistributor_index;
    }
    redistributor_indices
}

/// Configures interrupt-routing related flags in `scr_el3` bitflags.
///
/// While in NS-ELx:
/// - G0 and G1s are signalled as FIQs and should go through EL3.
/// - G1ns are signalled as IRQs and should be handled without a world switch.
///
/// While in S-ELx:
/// - G1s are signalled as IRQs and should be handled without a world switch.
/// - G0 and G1ns are signalled as FIQs and should not be routed until execution goes back to the NS world.
pub fn set_routing_model(scr_el3: &mut ScrEl3, world: World) {
    match world {
        World::NonSecure => {
            *scr_el3 |= ScrEl3::FIQ;
            *scr_el3 -= ScrEl3::IRQ;
        }
        World::Secure => {
            *scr_el3 -= ScrEl3::IRQ;
            *scr_el3 -= ScrEl3::FIQ;
        }
        #[cfg(feature = "rme")]
        World::Realm => todo!("Routing model for Realms not configured."),
    }
}

/// Returns the type of the highest priority pending group0 interrupt.
pub fn get_pending_interrupt_type() -> InterruptType {
    let int_id = GicV3::get_pending_interrupt(InterruptGroup::Group0);

    match int_id {
        None => InterruptType::Invalid,
        Some(IntId::SPECIAL_SECURE) => InterruptType::Secure,
        Some(IntId::SPECIAL_NONSECURE) => InterruptType::NonSecure,
        Some(_) => InterruptType::El3,
    }
}

/// Wraps a platform-specific group 0 interrupt handler.
pub fn handle_group0_interrupt() {
    let int_id = GicV3::get_and_acknowledge_interrupt(InterruptGroup::Group0).unwrap();
    debug!("Group 0 interrupt {int_id:?} acknowledged");

    PlatformImpl::handle_group0_interrupt(int_id);

    GicV3::end_interrupt(int_id, InterruptGroup::Group0);
    debug!("Group 0 interrupt {int_id:?} EOI");
}
