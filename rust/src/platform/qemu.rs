// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    logger,
    pagetable::{map_region, MT_DEVICE},
    pl011::Uart,
};
use aarch64_paging::{idmap::IdMap, paging::MemoryRegion};
use log::LevelFilter;

const DEVICE0_BASE: usize = 0x0800_0000;
const DEVICE0_SIZE: usize = 0x0100_0000;
const DEVICE1_BASE: usize = 0x0900_0000;
const DEVICE1_SIZE: usize = 0x00c0_0000;
pub const BL31_BASE: usize = BL31_LIMIT - 0x6_0000;
const BL31_LIMIT: usize = BL_RAM_BASE + BL_RAM_SIZE - FW_HANDOFF_SIZE;
const BL_RAM_BASE: usize = SHARED_RAM_BASE + SHARED_RAM_SIZE;
const BL_RAM_SIZE: usize = SEC_SRAM_SIZE - SHARED_RAM_SIZE;
const FW_HANDOFF_SIZE: usize = 0;
const SEC_SRAM_BASE: usize = 0x0e00_0000;
const SEC_SRAM_SIZE: usize = 0x0010_0000;
const SHARED_RAM_BASE: usize = SEC_SRAM_BASE;
const SHARED_RAM_SIZE: usize = 0x0000_1000;
const SHARED_RAM: MemoryRegion =
    MemoryRegion::new(SHARED_RAM_BASE, SHARED_RAM_BASE + SHARED_RAM_SIZE);
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);
const DEVICE1: MemoryRegion = MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE);

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: *mut u32 = 0x0900_0000 as _;

/// The aarch64 'virt' machine of the QEMU emulator.
pub struct Qemu;

impl Platform for Qemu {
    const CORE_COUNT: usize = 4;

    fn init_beforemmu() {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range.
        let uart = unsafe { Uart::new(PL011_BASE_ADDRESS) };
        logger::init(uart, LevelFilter::Trace).expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        map_region(idmap, &DEVICE0, MT_DEVICE);
        map_region(idmap, &DEVICE1, MT_DEVICE);
    }
}
