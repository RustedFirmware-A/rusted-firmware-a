// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::layout::{bl31_end, bl_code_base, bl_code_end, bl_ro_data_base, bl_ro_data_end};
use aarch64_paging::{
    idmap::IdMap,
    paging::{Attributes, MemoryRegion, PhysicalAddress, TranslationRegime},
};
use log::info;

const ROOT_LEVEL: usize = 1;

// Indices of entries in the Memory Attribute Indirection Register.
const MAIR_IWBWA_OWBWA_NTR_INDEX: usize = 0;
const MAIR_DEVICE_INDEX: usize = 1;
const MAIR_NON_CACHEABLE_INDEX: usize = 2;

/// Device-nGnRE memory
const MAIR_DEV_NGNRE: u64 = 0x4;
/// Normal memory, Non-Cacheable
const MAIR_NORM_NC: u64 = 0x4;
/// Normal memory, write-back non-transient, read-write-allocate
const MAIR_NORM_WB_NTR_RWA: u64 = 0xf;
/// Bit shift for outer memory attributes for normal memory.
const MAIR_NORM_OUTER_SHIFT: usize = 4;

// Values for MAIR entries.
const MAIR_DEVICE: u64 = MAIR_DEV_NGNRE;
const MAIR_IWBWA_OWBWA_NTR: u64 =
    MAIR_NORM_WB_NTR_RWA | MAIR_NORM_WB_NTR_RWA << MAIR_NORM_OUTER_SHIFT;
const MAIR_NON_CACHEABLE: u64 = MAIR_NORM_NC | MAIR_NORM_NC << MAIR_NORM_OUTER_SHIFT;

// Attribute values corresponding to the above MAIR indices.
const IWBWA_OWBWA_NTR: Attributes = Attributes::ATTRIBUTE_INDEX_0;
const DEVICE: Attributes = Attributes::ATTRIBUTE_INDEX_1;
const NON_CACHEABLE: Attributes = Attributes::ATTRIBUTE_INDEX_2;

/// Attributes used for all mappings.
///
/// We always set the access flag, as we don't manage access flag faults. The `USER` bit is RES1 for
/// the EL3 translation regime.
const BASE: Attributes = Attributes::ACCESSED
    .union(Attributes::USER)
    .union(Attributes::VALID);

/// Attributes used for device mappings.
///
/// Device memory is always mapped as execute-never to avoid the possibility of a speculative
/// instruction fetch, which could be an issue if the memory region corresponds to a read-sensitive
/// peripheral.
const MT_DEVICE: Attributes = DEVICE
    .union(Attributes::OUTER_SHAREABLE)
    .union(BASE)
    .union(Attributes::UXN);

/// Attributes used for non-cacheable memory mappings.
const MT_NON_CACHEABLE: Attributes = NON_CACHEABLE.union(Attributes::OUTER_SHAREABLE).union(BASE);
/// Attributes used for regular memory mappings.
const MT_MEMORY: Attributes = IWBWA_OWBWA_NTR.union(BASE); // TODO: Sharability

/// Attributes used for code (i.e. text) mappings.
const MT_CODE: Attributes = MT_MEMORY.union(Attributes::READ_ONLY);

/// Attributes used for read-only data mappings.
const MT_RO_DATA: Attributes = MT_MEMORY
    .union(Attributes::READ_ONLY)
    .union(Attributes::UXN);

/// Attributes used for read-write data mappings.
const MT_RW_DATA: Attributes = MT_MEMORY.union(Attributes::UXN);

const SEC_SRAM_BASE: usize = 0x0e00_0000;
const SHARED_RAM_BASE: usize = SEC_SRAM_BASE;
const SHARED_RAM_SIZE: usize = 0x0000_1000;
const DEVICE0_BASE: usize = 0x0800_0000;
const DEVICE0_SIZE: usize = 0x0100_0000;
const DEVICE1_BASE: usize = 0x0900_0000;
const DEVICE1_SIZE: usize = 0x00c0_0000;
const BL31_BASE: usize = BL31_LIMIT - 0x6_0000;
const BL31_LIMIT: usize = BL_RAM_BASE + BL_RAM_SIZE - FW_HANDOFF_SIZE;
const BL_RAM_BASE: usize = SHARED_RAM_BASE + SHARED_RAM_SIZE;
const BL_RAM_SIZE: usize = SEC_SRAM_SIZE - SHARED_RAM_SIZE;
const SEC_SRAM_SIZE: usize = 0x0010_0000;
const FW_HANDOFF_SIZE: usize = 0;

#[no_mangle]
static mut mmu_cfg_params: MmuCfgParams = MmuCfgParams {
    mair: 0,
    tcr: 0,
    ttbr0: 0,
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(C)]
struct MmuCfgParams {
    mair: u64,
    tcr: u64,
    ttbr0: usize,
}

pub fn init() -> IdMap {
    let mut idmap = IdMap::new(0, ROOT_LEVEL, TranslationRegime::El3);

    // Corresponds to `bl_regions` in C TF-A, `plat/arm/common/arm_bl31_setup.c`.
    // BL31_TOTAL
    map_region(
        &mut idmap,
        &MemoryRegion::new(BL31_BASE, bl31_end()),
        MT_MEMORY,
    );
    // BL31_RO
    map_region(
        &mut idmap,
        &MemoryRegion::new(bl_code_base(), bl_code_end()),
        MT_CODE,
    );
    map_region(
        &mut idmap,
        &MemoryRegion::new(bl_ro_data_base(), bl_ro_data_end()),
        MT_RO_DATA,
    );

    // Corresponds to `plat_regions` in C TF-A.
    const SHARED_RAM: MemoryRegion =
        MemoryRegion::new(SHARED_RAM_BASE, SHARED_RAM_BASE + SHARED_RAM_SIZE);
    const SHARED_RAM_FLAGS: Attributes = MT_DEVICE;
    const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);
    const DEVICE1: MemoryRegion = MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE);
    map_region(&mut idmap, &SHARED_RAM, SHARED_RAM_FLAGS);
    map_region(&mut idmap, &DEVICE0, MT_DEVICE);
    map_region(&mut idmap, &DEVICE1, MT_DEVICE);

    info!("Setting MMU config");
    setup_mmu_cfg(idmap.root_address());
    unsafe {
        enable_mmu_direct_el3(0);
    }
    info!("Marking page table as active");
    idmap.mark_active(0);

    idmap
}

/// Adds the given region to the page table with the given attributes, logging it first.
fn map_region(idmap: &mut IdMap, region: &MemoryRegion, attributes: Attributes) {
    info!("Mapping {} as {:?}.", region, attributes);
    idmap
        .map_range(region, attributes)
        .expect("Error mapping memory range");
}

fn setup_mmu_cfg(root_address: PhysicalAddress) {
    let mair = MAIR_DEVICE << (MAIR_DEVICE_INDEX << 3)
        | MAIR_IWBWA_OWBWA_NTR << (MAIR_IWBWA_OWBWA_NTR_INDEX << 3)
        | MAIR_NON_CACHEABLE << (MAIR_NON_CACHEABLE_INDEX << 3);
    let tcr = 0b101 << 16 // 48 bit physical address size (256 TiB).
        | 64 - 39; // Size offset is 2**39 bytes (512 GiB).
    let ttbr0 = root_address.0;

    unsafe {
        mmu_cfg_params.mair = mair;
        mmu_cfg_params.tcr = tcr;
        mmu_cfg_params.ttbr0 = ttbr0;
    }
}

extern "C" {
    fn enable_mmu_direct_el3(flags: u32);
}
