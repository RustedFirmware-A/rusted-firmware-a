// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    aarch64::{dsb_sy, wfi},
    context::EntryPointInfo,
    gicv3, logger,
    pagetable::{map_region, IdMap, MT_DEVICE},
    semihosting::{semihosting_exit, AdpStopped},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, Psci, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures,
        },
    },
    sysregs::Spsr,
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::{
    gicv3::{GicV3, SecureIntGroup},
    {IntId, Trigger},
};
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use arm_psci::{ErrorCode, Mpidr, PowerState};
use core::ptr::NonNull;
use gicv3::{GicConfig, SecureInterruptConfig};
use log::LevelFilter;
use percore::Cores;

const DEVICE0_BASE: usize = 0x0800_0000;
const DEVICE0_SIZE: usize = 0x0100_0000;
const DEVICE1_BASE: usize = 0x0900_0000;
const DEVICE1_SIZE: usize = 0x00c0_0000;
const SEC_SRAM_BASE: usize = 0x0e00_0000;
const SHARED_RAM_BASE: usize = SEC_SRAM_BASE;
const SHARED_RAM_SIZE: usize = 0x0000_1000;
const SHARED_RAM: MemoryRegion =
    MemoryRegion::new(SHARED_RAM_BASE, SHARED_RAM_BASE + SHARED_RAM_SIZE);
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);
const DEVICE1: MemoryRegion = MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE);

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: *mut PL011Registers = 0x0900_0000 as _;
/// Base address of GICv3 distributor.
const GICD_BASE_ADDRESS: *mut u64 = 0x800_0000 as _;
/// Base address of the first GICv3 redistributor frame.
const GICR_BASE_ADDRESS: *mut u64 = 0x80A_0000 as _;
/// Size of a single GIC redistributor frame (there is one per core).
// TODO: Maybe GIC should infer the frame size based on info gicv3 vs gicv4.
// Because I think only 1 << 0x11 and 1 << 0x12 values are allowed.
const GICR_FRAME_SIZE: usize = 1 << 0x11;

// TODO: Use the correct addresses here.
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0;
const HW_CONFIG_ADDRESS: u64 = 0;

/// The number of CPU clusters.
const CLUSTER_COUNT: usize = 1;
/// The maximum number of CPUs in each cluster.
const MAX_CPUS_PER_CLUSTER: usize = 4;

/// The aarch64 'virt' machine of the QEMU emulator.
pub struct Qemu;

impl Platform for Qemu {
    const CORE_COUNT: usize = CLUSTER_COUNT * MAX_CPUS_PER_CLUSTER;

    type LoggerWriter = Uart<'static>;
    type PsciPlatformImpl = QemuPsciPlatformImpl;

    const GIC_CONFIG: GicConfig = GicConfig {
        // TODO: Fill this with proper values.
        secure_interrupts_config: &[SecureInterruptConfig {
            id: IntId::spi(0),
            priority: 0x81,
            group: SecureIntGroup::Group1S,
            trigger: Trigger::Level,
        }],
    };

    fn init_beforemmu() {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range. The address remains valid after turning on the MMU
        // because of the identity mapping of the `DEVICE1` region.
        let uart_pointer =
            unsafe { UniqueMmioPointer::new(NonNull::new(PL011_BASE_ADDRESS).unwrap()) };
        logger::init(Uart::new(uart_pointer), LevelFilter::Trace)
            .expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        map_region(idmap, &DEVICE0, MT_DEVICE);
        map_region(idmap, &DEVICE1, MT_DEVICE);
    }

    unsafe fn create_gic() -> GicV3 {
        // SAFETY: `GICD_BASE_ADDRESS` and `GICR_BASE_ADDRESS` are base addresses of a GIC device,
        // and nothing else accesses that address range.
        // TODO: Powering on-off secondary cores will also access their GIC Redistributors.
        unsafe {
            GicV3::new(
                GICD_BASE_ADDRESS,
                GICR_BASE_ADDRESS,
                Qemu::CORE_COUNT,
                GICR_FRAME_SIZE,
            )
        }
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = Psci::core_index() as u64;
        EntryPointInfo {
            pc: 0x0e10_0000,
            #[cfg(feature = "sel2")]
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            #[cfg(not(feature = "sel2"))]
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL1H,
            args: [
                TOS_FW_CONFIG_ADDRESS,
                HW_CONFIG_ADDRESS,
                0,
                0,
                core_linear_id,
                0,
                0,
                0,
            ],
        }
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x6000_0000,
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            args: Default::default(),
        }
    }

    fn psci_platform() -> Option<Self::PsciPlatformImpl> {
        Some(QemuPsciPlatformImpl)
    }

    fn arch_workaround_1_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_1() {}

    fn arch_workaround_2_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_2() {}

    fn arch_workaround_3_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_3() {}

    fn arch_workaround_4_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }
}

#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum QemuPowerState {
    PowerDown,
    Standby,
    On,
}

impl PlatformPowerStateInterface for QemuPowerState {
    const OFF: Self = Self::PowerDown;
    const RUN: Self = Self::On;

    fn power_state_type(&self) -> PowerStateType {
        match self {
            Self::PowerDown => PowerStateType::PowerDown,
            Self::Standby => PowerStateType::StandbyOrRetention,
            Self::On => PowerStateType::Run,
        }
    }
}

impl From<QemuPowerState> for usize {
    fn from(_value: QemuPowerState) -> Self {
        todo!()
    }
}

pub struct QemuPsciPlatformImpl;

// SAFETY: The implementation of `try_get_cpu_index_by_mpidr` never returns the same index for
// different cores because each core has a cluster ID and CPU ID in its MPIDR, and we have a
// suitable MAX_CPUS_PER_CLUSTER value to avoid overlap.
unsafe impl PsciPlatformInterface for QemuPsciPlatformImpl {
    const POWER_DOMAIN_COUNT: usize = 1 + CLUSTER_COUNT + Qemu::CORE_COUNT;
    const MAX_POWER_LEVEL: usize = 2;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::empty();

    type PlatformPowerState = QemuPowerState;

    fn topology() -> &'static [usize] {
        &[1, CLUSTER_COUNT, MAX_CPUS_PER_CLUSTER]
    }

    fn try_parse_power_state(_power_state: PowerState) -> Option<PsciCompositePowerState> {
        todo!()
    }

    fn cpu_standby(&self, cpu_state: QemuPowerState) {
        assert_eq!(cpu_state, QemuPowerState::Standby);

        dsb_sy();
        wfi();
    }

    fn power_domain_suspend(&self, _target_state: &PsciCompositePowerState) {
        todo!()
    }

    fn power_domain_suspend_finish(&self, _target_state: &PsciCompositePowerState) {
        todo!()
    }

    fn power_domain_off(&self, _target_state: &PsciCompositePowerState) {
        todo!()
    }

    fn power_domain_on(&self, _mpidr: Mpidr) -> Result<(), ErrorCode> {
        todo!()
    }

    fn power_domain_on_finish(&self, _target_state: &PsciCompositePowerState) {
        todo!()
    }

    fn system_off(&self) -> ! {
        semihosting_exit(AdpStopped::ApplicationExit, 0);
        panic!("Semihosting system off call unexpectedly returned.");
    }

    fn system_reset(&self) -> ! {
        todo!()
    }

    fn try_get_cpu_index_by_mpidr(mpidr: Mpidr) -> Option<usize> {
        // TODO: Ensure that this logic is always the same as the assembly `plat_my_core_pos` /
        // `plat_qemu_calc_core_pos`. Can they be combined somehow? The assembly version is needed
        // because it is called from `plat_get_my_stack` before the stack is set up.
        let cluster_id = usize::from(mpidr.aff1);
        let cpu_id = usize::from(mpidr.aff0);
        if cluster_id < CLUSTER_COUNT && cpu_id < MAX_CPUS_PER_CLUSTER {
            Some(cluster_id * MAX_CPUS_PER_CLUSTER + cpu_id)
        } else {
            None
        }
    }
}
