// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    layout::{bl31_end, bl_code_base, bl_code_end, bl_ro_data_base, bl_ro_data_end},
    platform::{Platform, PlatformImpl, BL31_BASE},
};
use aarch64_paging::{
    idmap::IdMap,
    paging::{Attributes, MemoryRegion, PhysicalAddress, TranslationRegime},
};
use log::info;

const ROOT_LEVEL: usize = 1;

// Indices of entries in the Memory Attribute Indirection Register.
const MAIR_IWBRWA_OWBRWA_NTR_INDEX: usize = 0;
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
const MAIR_IWBRWA_OWBRWA_NTR: u64 =
    MAIR_NORM_WB_NTR_RWA | MAIR_NORM_WB_NTR_RWA << MAIR_NORM_OUTER_SHIFT;
const MAIR_NON_CACHEABLE: u64 = MAIR_NORM_NC | MAIR_NORM_NC << MAIR_NORM_OUTER_SHIFT;

// Attribute values corresponding to the above MAIR indices.
const IWBRWA_OWBRWA_NTR: Attributes = Attributes::ATTRIBUTE_INDEX_0;
const DEVICE: Attributes = Attributes::ATTRIBUTE_INDEX_1;
#[allow(unused)]
const NON_CACHEABLE: Attributes = Attributes::ATTRIBUTE_INDEX_2;

/// Attribute bits which are RES1 for the EL3 translation regime, as we configure it.
const EL3_RES1: Attributes = Attributes::USER;

/// Attributes used for all mappings.
///
/// We always set the access flag, as we don't manage access flag faults.
const BASE: Attributes = EL3_RES1
    .union(Attributes::ACCESSED)
    .union(Attributes::VALID);

/// Attributes used for device mappings.
///
/// Device memory is always mapped as execute-never to avoid the possibility of a speculative
/// instruction fetch, which could be an issue if the memory region corresponds to a read-sensitive
/// peripheral.
pub const MT_DEVICE: Attributes = DEVICE
    .union(Attributes::OUTER_SHAREABLE)
    .union(BASE)
    .union(Attributes::UXN);

/// Attributes used for non-cacheable memory mappings.
#[allow(unused)]
pub const MT_NON_CACHEABLE: Attributes =
    NON_CACHEABLE.union(Attributes::OUTER_SHAREABLE).union(BASE);
/// Attributes used for regular memory mappings.
pub const MT_MEMORY: Attributes = IWBRWA_OWBRWA_NTR.union(BASE); // TODO: Sharability

/// Attributes used for code (i.e. text) mappings.
pub const MT_CODE: Attributes = MT_MEMORY.union(Attributes::READ_ONLY);

/// Attributes used for read-only data mappings.
pub const MT_RO_DATA: Attributes = MT_MEMORY
    .union(Attributes::READ_ONLY)
    .union(Attributes::UXN);

/// Attributes used for read-write data mappings.
#[allow(unused)]
pub const MT_RW_DATA: Attributes = MT_MEMORY.union(Attributes::UXN);

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
    PlatformImpl::map_extra_regions(&mut idmap);

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
pub fn map_region(idmap: &mut IdMap, region: &MemoryRegion, attributes: Attributes) {
    info!("Mapping {} as {:?}.", region, attributes);
    idmap
        .map_range(region, attributes)
        .expect("Error mapping memory range");
}

fn setup_mmu_cfg(root_address: PhysicalAddress) {
    let mair = MAIR_DEVICE << (MAIR_DEVICE_INDEX << 3)
        | MAIR_IWBRWA_OWBRWA_NTR << (MAIR_IWBRWA_OWBRWA_NTR_INDEX << 3)
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
