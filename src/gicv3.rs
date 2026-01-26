// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::{panic, ptr::NonNull};

use crate::{
    aarch64::{dsb_sy, isb},
    context::{CoresImpl, World},
    platform::{Platform, PlatformImpl},
};
use arm_gic::{
    IntId, Trigger, UniqueMmioPointer,
    gicv3::{
        GicCpuInterface, GicDistributor, GicDistributorContext, GicRedistributor,
        GicRedistributorContext, GicRedistributorIterator, Group, HIGHEST_NS_PRIORITY,
        InterruptGroup, SecureIntGroup,
        registers::{Gicd, GicdCtlr, GicrSgi},
    },
};
use arm_sysregs::{MpidrEl1, ScrEl3, read_mpidr_el1};
use log::debug;
use percore::Cores;
use spin::{Once, mutex::SpinMutex};

const GIC_PRI_MASK: u8 = 0xff;

static GIC: Once<Gic> = Once::new();

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

impl InterruptConfig {
    /// `default()` from [Default] trait is not const.
    pub const DEFAULT: Self = Self {
        priority: HIGHEST_NS_PRIORITY,
        group: Group::Group1NS,
        trigger: Trigger::Level,
    };
}

impl Default for InterruptConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

pub type InterruptConfigEntry = (IntId, InterruptConfig);

/// The configuration of platform's GIC.
pub struct GicConfig {
    /// This list specifies which interrupts will be configured to specified setup and enabled by
    /// EL3.
    pub interrupts_config: &'static [InterruptConfigEntry],
}

impl GicConfig {
    /// Get iterator for shared interrupts.
    fn shared(&self) -> impl Iterator<Item = &InterruptConfigEntry> {
        self.interrupts_config.iter().filter(|int| int.0.is_spi())
    }

    /// Get iterator for private interrupts.
    fn private(&self) -> impl Iterator<Item = &InterruptConfigEntry> {
        self.interrupts_config
            .iter()
            .filter(|int| int.0.is_private())
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

/// Registry for storing GIC redistributor instances.
struct GicRedistributorRegistry<'a> {
    redistributors: [SpinMutex<GicRedistributor<'a>>; PlatformImpl::CORE_COUNT],
}

impl<'a> GicRedistributorRegistry<'a> {
    /// # Safety
    /// The caller must ensure that `base` points to a continiously mapped GIC redistributor memory
    /// area that spans until the last redistributor block where GICR_TYPER.Last is set. There must
    /// be no other references to this address.
    pub unsafe fn new(base: NonNull<GicrSgi>, gic_v4: bool) -> Self {
        let mut redistributors = [const { None }; PlatformImpl::CORE_COUNT];

        // Safety: The function propagates the safety requirements to the caller.
        for redist in unsafe { GicRedistributorIterator::new(base, gic_v4) } {
            let mpidr = MpidrEl1::from_psci_mpidr(redist.typer().core_mpidr());
            assert!(PlatformImpl::mpidr_is_valid(mpidr));

            let core_index = PlatformImpl::core_position(mpidr.bits());

            redistributors[core_index] = Some(SpinMutex::new(redist));
        }

        Self {
            redistributors: redistributors.map(|r| r.unwrap()),
        }
    }

    /// Get redistributor by linear index.
    pub fn redistributor(&self, index: usize) -> &SpinMutex<GicRedistributor<'a>> {
        &self.redistributors[index]
    }

    /// Get the redistributor of the local core.
    pub fn local_redistributor(&self) -> &SpinMutex<GicRedistributor<'a>> {
        self.redistributor(CoresImpl::core_index())
    }
}

/// The `Gic` structure contains the driver instances of the GIC distributor and redistributors. Its
/// implementation offers platform independent functions for initializing, enabling, disabling,
/// saving and restoring the various components of the GIC peripheral.
pub struct Gic<'a> {
    distributor: SpinMutex<GicDistributor<'a>>,
    redistributors: GicRedistributorRegistry<'a>,
}

impl<'a> Gic<'a> {
    /// # Safety
    /// The caller must ensure that `gicr_base` points to a continiously mapped GIC redistributor
    /// memory area that spans until the last redistributor block where GICR_TYPER.Last is set.
    /// There must be no other references to this address range.
    pub unsafe fn new(
        gicd: UniqueMmioPointer<'a, Gicd>,
        gicr_base: NonNull<GicrSgi>,
        gic_v4: bool,
    ) -> Self {
        Self {
            distributor: SpinMutex::new(GicDistributor::new(gicd)),
            // Safety:  Our caller promised that `gicr_base` is a valid and unique pointer to a GIC
            // redistributor block.
            redistributors: unsafe { GicRedistributorRegistry::new(gicr_base, gic_v4) },
        }
    }

    /// Get GIC instance.
    pub fn get() -> &'static Self {
        GIC.get().unwrap()
    }

    /// Sets the default configuration for all interrupts of the distributor. Configures the shared
    /// interrupts that were specificied in the `GicConfig` and enables the required interrupt
    /// groups.
    pub fn distributor_init(&self, config: &GicConfig) {
        let mut distributor = self.distributor.lock();

        // Clear the "enable" bits for G0/G1S/G1NS interrupts before configuring the ARE_S bit. The
        // Distributor might generate a system error otherwise.
        distributor.modify_control(
            GicdCtlr::EnableGrp0 | GicdCtlr::EnableGrp1S | GicdCtlr::EnableGrp1NS,
            false,
        );

        // Set the ARE_S and ARE_NS bit now that interrupts have been disabled
        distributor.modify_control(GicdCtlr::ARE_S | GicdCtlr::ARE_NS, true);

        // Set the default attribute of all (E)SPIs
        distributor.configure_default_settings();

        let mpidr = Some(read_mpidr_el1().bits());

        for (intid, config) in config.shared() {
            distributor.set_group(*intid, config.group).unwrap();
            distributor.set_trigger(*intid, config.trigger).unwrap();
            distributor
                .set_interrupt_priority(*intid, config.priority)
                .unwrap();
            distributor.set_routing(*intid, mpidr).unwrap();
            distributor.enable_interrupt(*intid, true).unwrap();
        }

        let gicd_ctlr = config
            .shared()
            .fold(GicdCtlr::empty(), |acc, (_intid, config)| {
                acc | match config.group {
                    Group::Secure(SecureIntGroup::Group0) => GicdCtlr::EnableGrp0,
                    Group::Secure(SecureIntGroup::Group1S) => GicdCtlr::EnableGrp1S,
                    Group::Group1NS => panic!("configuring Group1NS is not permitted"),
                }
            });

        distributor.modify_control(gicd_ctlr, true);
    }

    /// Saves the distributor context.
    #[allow(unused)]
    pub fn distributor_save<const IREG_COUNT: usize, const IREG_E_COUNT: usize>(
        &self,
        context: &mut GicDistributorContext<IREG_COUNT, IREG_E_COUNT>,
    ) {
        self.distributor.lock().save(context).unwrap();
    }

    /// Restores the distributor context.
    #[allow(unused)]
    pub fn distributor_restore<const IREG_COUNT: usize, const IREG_E_COUNT: usize>(
        &self,
        context: &GicDistributorContext<IREG_COUNT, IREG_E_COUNT>,
    ) {
        self.distributor.lock().restore(context).unwrap()
    }

    /// Powers on the redistributor instance of the local core, then sets the default configuration
    /// for all interrupts of the redistributor. Configures the private interrupts that were
    /// specified in the `GicConfig`.
    pub fn redistributor_init(&self, config: &GicConfig) {
        let mut redist = self.redistributors.local_redistributor().lock();

        redist.power_on();

        redist.configure_default_settings();

        for (intid, config) in config.private() {
            redist.set_group(*intid, config.group).unwrap();
            if intid.is_ppi() {
                // Set interrupt configuration for PPIs.
                // Configurations for SGIs 0-15 are ignored.
                redist.set_trigger(*intid, config.trigger).unwrap();
            }
            redist
                .set_interrupt_priority(*intid, config.priority)
                .unwrap();
            redist.enable_interrupt(*intid, true).unwrap();
        }
    }

    /// Turns off the local core's redistributor.
    #[allow(unused)]
    pub fn redistributor_off(&self) {
        self.redistributors.local_redistributor().lock().power_off();
    }

    /// Saves the context of the local core's redistributor.
    #[allow(unused)]
    pub fn redistributor_save<const IREG_COUNT: usize>(
        &self,
        context: &mut GicRedistributorContext<IREG_COUNT>,
    ) {
        self.redistributors
            .local_redistributor()
            .lock()
            .save(context)
            .unwrap()
    }

    /// Restores the context of the local core's redistributor.
    #[allow(unused)]
    pub fn redistributor_restore<const IREG_COUNT: usize>(
        &self,
        context: &GicRedistributorContext<IREG_COUNT>,
    ) {
        let mut redistributor = self.redistributors.local_redistributor().lock();

        redistributor.power_on();
        redistributor.restore(context).unwrap();
    }

    /// Enables and configures the GIC CPU interface.
    pub fn cpu_interface_enable(&self) {
        let mut redist = self.redistributors.local_redistributor().lock();
        redist.mark_core_awake().unwrap();

        GicCpuInterface::disable_legacy_interrupt_bypass_el3(true);

        // Enable system register access for EL3 and allow lower exception
        // levels to configure the same for themselves. If the legacy mode is
        // not supported, the SRE bit is RAO/WI
        GicCpuInterface::enable_system_register_el3(true, true);

        // Prevent the selection of legacy mode where Secure Group 1 interrupts are treated as Group 0.
        GicCpuInterface::enable_system_register_el1(true);
        isb();

        GicCpuInterface::set_priority_mask(GIC_PRI_MASK);
        GicCpuInterface::enable_group0(true);
        GicCpuInterface::enable_group1(true);

        isb();
        dsb_sy();
    }

    /// Disables the GIC CPU interface.
    #[allow(unused)]
    pub fn cpu_interface_disable(&self) {
        GicCpuInterface::enable_group0(false);
        GicCpuInterface::enable_group1_secure(false);
        GicCpuInterface::enable_group1_non_secure(false);

        isb();
        dsb_sy();

        // dsb() already issued previously after clearing the CPU group enabled, apply below
        // workaround to toggle the "DPG*" bits of GICR_CTLR register for unblocking event.
        // TODO: gicv3_apply_errata_wa_2384374(gicr_base);

        let mut redist = self.redistributors.local_redistributor().lock();
        redist.mark_core_asleep().unwrap();
    }
}

/// Initializes the gic by configuring the distributor, redistributor and cpu interface, and puts
/// the global gic into GIC variable. This function should only be called once early in the boot
/// process. Subsequent calls will be ignored.
pub fn init() {
    GIC.call_once(|| {
        // SAFETY: This is the only place where GIC is created and there are no aliases.
        let gic = unsafe { PlatformImpl::create_gic() };

        gic.distributor_init(&PlatformImpl::GIC_CONFIG);
        gic.redistributor_init(&PlatformImpl::GIC_CONFIG);
        gic.cpu_interface_enable();

        gic
    });
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
        World::Realm => {
            *scr_el3 -= ScrEl3::FIQ;
            *scr_el3 -= ScrEl3::IRQ;
        }
    }
}

/// Returns the type of the highest priority pending group0 interrupt.
pub fn get_pending_interrupt_type() -> InterruptType {
    let int_id = GicCpuInterface::get_pending_interrupt(InterruptGroup::Group0);

    match int_id {
        None => InterruptType::Invalid,
        Some(IntId::SPECIAL_SECURE) => InterruptType::Secure,
        Some(IntId::SPECIAL_NONSECURE) => InterruptType::NonSecure,
        Some(_) => InterruptType::El3,
    }
}

/// Wraps a platform-specific group 0 interrupt handler.
pub fn handle_group0_interrupt() {
    let int_id = GicCpuInterface::get_and_acknowledge_interrupt(InterruptGroup::Group0).unwrap();

    debug!("Group 0 interrupt {int_id:?} acknowledged");

    PlatformImpl::handle_group0_interrupt(int_id);

    GicCpuInterface::end_interrupt(int_id, InterruptGroup::Group0);
    debug!("Group 0 interrupt {int_id:?} EOI");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_save_restore_off() {
        GIC.call_once(|| {
            // SAFETY: The `GIC.call_once` ensures this isn't called multiple times.
            unsafe { PlatformImpl::create_gic() }
        });

        let gic = Gic::get();
        let mut distributor_context = GicDistributorContext::<
            { GicDistributorContext::ireg_count(988) },
            { GicDistributorContext::ireg_e_count(1024) },
        >::new();
        gic.distributor_save(&mut distributor_context);
        gic.distributor_restore(&distributor_context);
        let mut redistributor_context =
            GicRedistributorContext::<{ GicRedistributorContext::ireg_count(96) }>::new();
        gic.redistributor_save(&mut redistributor_context);
        gic.redistributor_restore(&redistributor_context);
        gic.redistributor_off();
    }
}
