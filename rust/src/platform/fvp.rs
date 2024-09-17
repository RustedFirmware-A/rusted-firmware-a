// Copyright (c) 2024, Arm Ltd. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

include!("../../platforms/fvp/config.rs");

use super::Platform;
use crate::{
    context::EntryPointInfo,
    gicv3, logger,
    pagetable::{map_region, IdMap, MT_DEVICE},
    services::arch::WorkaroundSupport,
    sysregs::SpsrEl3,
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::{
    gicv3::{GicV3, SecureIntGroup},
    IntId, Trigger,
};
use arm_pl011_uart::{OwnedMmioPointer, PL011Registers, Uart};
use core::ptr::NonNull;
use gicv3::SecureInterruptConfig;
use log::LevelFilter;
use percore::Cores;

const BASE_GICD_BASE: usize = 0x2f00_0000;
const BASE_GICR_BASE: usize = 0x2f10_0000;
/// Size of a single GIC redistributor frame (there is one per core).
// TODO: Maybe GIC should infer the frame size based on info gicv3 vs gicv4.
// Because I think only 1 << 0x11 and 1 << 0x12 values are allowed.
const GICR_FRAME_SIZE: usize = 1 << 0x11;

const DEVICE0_BASE: usize = 0x2000_0000;
const DEVICE0_SIZE: usize = 0x0c20_0000;
const DEVICE1_BASE: usize = BASE_GICD_BASE;
const PLATFORM_CORE_COUNT: usize =
    FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;
const DEVICE1_SIZE: usize = (BASE_GICR_BASE - BASE_GICD_BASE) + (PLATFORM_CORE_COUNT * 0x2_0000);

const ARM_TRUSTED_SRAM_BASE: usize = 0x0400_0000;
const ARM_SHARED_RAM_BASE: usize = ARM_TRUSTED_SRAM_BASE;
const ARM_SHARED_RAM_SIZE: usize = 0x0000_1000; /* 4 KB */

const V2M_IOFPGA_BASE: usize = 0x1c00_0000;
const V2M_IOFPGA_SIZE: usize = 0x0300_0000;

const SHARED_RAM: MemoryRegion = MemoryRegion::new(
    ARM_SHARED_RAM_BASE,
    ARM_SHARED_RAM_BASE + ARM_SHARED_RAM_SIZE,
);

const V2M_MAP_IOFPGA: MemoryRegion =
    MemoryRegion::new(V2M_IOFPGA_BASE, V2M_IOFPGA_BASE + V2M_IOFPGA_SIZE);

const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);
const DEVICE1: MemoryRegion = MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE);

// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: *mut PL011Registers = 0x1C09_0000 as _;

// TODO: Use the correct addresses here.
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0;
const HW_CONFIG_ADDRESS: u64 = 0;

// TODO: Use the correct values here (see services/std_svc/rmmd/rmmd_main.c).
/// Version of the RMM Boot Interface.
const RMM_BOOT_VERSION: u64 = 0;
/// Base address for the EL3 - RMM shared area. The boot manifest should be stored at the beginning
/// of this area.
const RMM_SHARED_AREA_BASE_ADDRESS: u64 = 0;

/// Fixed Virtual Platform
pub struct Fvp;

impl Platform for Fvp {
    const CORE_COUNT: usize = PLATFORM_CORE_COUNT;

    type LoggerWriter = Uart<'static>;

    const GIC_CONFIG: gicv3::GicConfig = gicv3::GicConfig {
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
        // because of the identity mapping of the `V2M_MAP_IOFPGA` region.
        let uart_pointer =
            unsafe { OwnedMmioPointer::new(NonNull::new(PL011_BASE_ADDRESS).unwrap()) };
        logger::init(Uart::new(uart_pointer), LevelFilter::Trace)
            .expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        map_region(idmap, &V2M_MAP_IOFPGA, MT_DEVICE);
        map_region(idmap, &DEVICE0, MT_DEVICE);
        map_region(idmap, &DEVICE1, MT_DEVICE);
    }

    unsafe fn create_gic() -> GicV3 {
        // SAFETY: `GICD_BASE_ADDRESS` and `GICR_BASE_ADDRESS` are base addresses of a GIC device,
        // and nothing else accesses that address range.
        // TODO: Powering on-off secondary cores will also access their GIC Redistributors.
        unsafe {
            GicV3::new(
                BASE_GICD_BASE as *mut u64,
                BASE_GICR_BASE as *mut u64,
                Fvp::CORE_COUNT,
                GICR_FRAME_SIZE,
            )
        }
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = Self::core_index() as u64;
        EntryPointInfo {
            pc: 0x0600_0000,
            #[cfg(feature = "sel2")]
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL2H,
            #[cfg(not(feature = "sel2"))]
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL1H,
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
            pc: 0x8800_0000,
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL2H,
            args: Default::default(),
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        let core_linear_id = Self::core_index() as u64;
        EntryPointInfo {
            pc: 0xfdc00000,
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL2H,
            args: [
                core_linear_id,
                RMM_BOOT_VERSION,
                Self::CORE_COUNT as u64,
                RMM_SHARED_AREA_BASE_ADDRESS,
                0,
                0,
                0,
                0,
            ],
        }
    }

    fn system_off() -> ! {
        unimplemented!("System off not implemented on FVP.")
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

// SAFETY: This implementation never returns the same index for different cores.
unsafe impl Cores for Fvp {
    fn core_index() -> usize {
        // TODO: Implement this properly. Ensure that the safety invariant still holds, and update
        // the comment to explain how.
        0
    }
}
