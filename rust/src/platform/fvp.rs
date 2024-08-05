// Copyright (c) 2024, Arm Ltd. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

mod fvp_defines {
    include!(concat!(env!("OUT_DIR"), "/fvp_defines.rs"));
}
use fvp_defines::{FVP_CLUSTER_COUNT, FVP_MAX_CPUS_PER_CLUSTER, FVP_MAX_PE_PER_CPU};

use super::Platform;
use crate::{
    context::EntryPointInfo,
    logger,
    pagetable::{map_region, IdMap, MT_DEVICE},
    pl011::Uart,
};
use aarch64_paging::paging::MemoryRegion;
use log::LevelFilter;
use percore::Cores;

const BASE_GICD_BASE: usize = 0x2f00_0000;
const BASE_GICR_BASE: usize = 0x2f10_0000;

const DEVICE0_BASE: usize = 0x2000_0000;
const DEVICE0_SIZE: usize = 0x0c20_0000;
const DEVICE1_BASE: usize = BASE_GICD_BASE;
const PLATFORM_CORE_COUNT: usize =
    FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;
const DEVICE1_SIZE: usize = (BASE_GICR_BASE - BASE_GICD_BASE) + (PLATFORM_CORE_COUNT * 0x2_0000);

const PLAT_ARM_MAX_BL31_SIZE: usize = PLAT_ARM_TRUSTED_SRAM_SIZE - ARM_SHARED_RAM_SIZE;

const PLAT_ARM_TRUSTED_SRAM_SIZE: usize = 256 * 1024;
const ARM_TRUSTED_SRAM_BASE: usize = 0x0400_0000;
const ARM_SHARED_RAM_BASE: usize = ARM_TRUSTED_SRAM_BASE;
const ARM_SHARED_RAM_SIZE: usize = 0x0000_1000; /* 4 KB */
const ARM_BL_RAM_BASE: usize = ARM_SHARED_RAM_BASE + ARM_SHARED_RAM_SIZE;
const ARM_BL_RAM_SIZE: usize = PLAT_ARM_TRUSTED_SRAM_SIZE - ARM_SHARED_RAM_SIZE;

pub const BL31_BASE: usize = (ARM_BL_RAM_BASE + ARM_BL_RAM_SIZE) - PLAT_ARM_MAX_BL31_SIZE;

const ARM_DRAM1_BASE: usize = 0x8000_0000;
const ARM_DRAM1_SIZE: usize = 0x8000_0000;

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
const PL011_BASE_ADDRESS: *mut u32 = 0x1C09_0000 as _;

/// Fixed Virtual Platform
pub struct Fvp;

impl Platform for Fvp {
    const CORE_COUNT: usize = PLATFORM_CORE_COUNT;

    fn init_beforemmu() {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range.
        let uart = unsafe { Uart::new(PL011_BASE_ADDRESS) };
        logger::init(uart, LevelFilter::Trace).expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        map_region(idmap, &V2M_MAP_IOFPGA, MT_DEVICE);
        map_region(idmap, &DEVICE0, MT_DEVICE);
        map_region(idmap, &DEVICE1, MT_DEVICE);
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x60000000,
            spsr: 0x04,
            args: Default::default(),
        }
    }

    fn system_off() -> ! {
        unimplemented!("System off not implemented on FVP.")
    }
}

unsafe impl Cores for Fvp {
    fn core_index() -> usize {
        // TODO: Implement this properly.
        0
    }
}
