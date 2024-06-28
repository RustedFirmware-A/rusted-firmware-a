// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    layout::{bl31_end, bl_code_base, bl_code_end, bl_ro_data_base, bl_ro_data_end},
    platform::{Platform, PlatformImpl, BL31_BASE},
};
use aarch64_paging::{
    paging::{
        Attributes, Constraints, MemoryRegion, PageTable, PhysicalAddress, Translation,
        TranslationRegime, VaRange, VirtualAddress,
    },
    MapError, Mapping,
};
use core::{mem::take, ptr::NonNull};
use log::{info, warn};
use spin::{
    mutex::{SpinMutex, SpinMutexGuard},
    Once,
};

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
///
/// From Arm ARM K.a, D8.3.1.2 Fig. D8-16: lower attributes AP[1] bit 6
/// and D8.4.1.2.1 Stage 1 data accesses using Direct permissions:
/// "For a stage 1 translation that supports one Exception level, AP[1] is RES1."
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
/// Arm ARM K.a D8.6.2 "If a region is mapped as Device memory or Normal
/// Non-cacheable memory after all enabled translation stages, then the
/// region has an effective Shareability attribute of Outer Shareable."
/// Arm ARM K.a D8.4.1.2.3 bit 54 UXN/PXN/XN is the XN field at EL3:
/// "If the Effective value of XN is 1, then PrivExecute is removed."
pub const MT_DEVICE: Attributes = DEVICE.union(BASE).union(Attributes::UXN);

/// Attributes used for non-cacheable memory mappings.
#[allow(unused)]
pub const MT_NON_CACHEABLE: Attributes = NON_CACHEABLE.union(BASE);

/// Attributes used for regular memory mappings.
pub const MT_MEMORY: Attributes = IWBRWA_OWBRWA_NTR
    .union(BASE)
    .union(Attributes::INNER_SHAREABLE);

/// Attributes used for code (i.e. text) mappings.
pub const MT_CODE: Attributes = MT_MEMORY.union(Attributes::READ_ONLY);

/// Attributes used for read-only data mappings.
pub const MT_RO_DATA: Attributes = MT_MEMORY
    .union(Attributes::READ_ONLY)
    .union(Attributes::UXN);

/// Attributes used for read-write data mappings.
#[allow(unused)]
pub const MT_RW_DATA: Attributes = MT_MEMORY.union(Attributes::UXN);

static PAGE_HEAP: SpinMutex<[PageTable; PlatformImpl::PAGE_HEAP_PAGE_COUNT]> =
    SpinMutex::new([PageTable::EMPTY; PlatformImpl::PAGE_HEAP_PAGE_COUNT]);
static PAGE_TABLE: Once<SpinMutex<IdMap>> = Once::new();

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

/// Initialises and enables the page table.
///
/// This should be called once early in startup, before anything else that depends on it.
pub fn init() {
    PAGE_TABLE.call_once(|| {
        let page_heap =
            SpinMutexGuard::leak(PAGE_HEAP.try_lock().expect("Page heap was already taken"));
        let mut idmap = IdMap::new(page_heap);

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
        idmap.mark_active();

        SpinMutex::new(idmap)
    });
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

struct IdTranslation {
    /// Pages which may be allocated for page tables but have not yet been.
    unused_pages: &'static mut [PageTable],
}

impl IdTranslation {
    fn virtual_to_physical(va: VirtualAddress) -> PhysicalAddress {
        // Physical address is the same as the virtual address because we are using identity mapping
        // everywhere.
        PhysicalAddress(va.0)
    }
}

impl Translation for IdTranslation {
    fn allocate_table(&mut self) -> (NonNull<PageTable>, PhysicalAddress) {
        let (table, rest) = take(&mut self.unused_pages)
            .split_first_mut()
            .expect("Failed to allocate page table");
        self.unused_pages = rest;

        let table = NonNull::from(table);
        (
            table,
            Self::virtual_to_physical(VirtualAddress(table.as_ptr() as usize)),
        )
    }

    unsafe fn deallocate_table(&mut self, page_table: NonNull<PageTable>) {
        warn!("Leaking page table allocation {:?}", page_table);
    }

    fn physical_to_virtual(&self, page_table_pa: PhysicalAddress) -> NonNull<PageTable> {
        NonNull::new(page_table_pa.0 as *mut PageTable)
            .expect("Got physical address 0 for pagetable")
    }
}

pub struct IdMap {
    mapping: Mapping<IdTranslation>,
}

impl IdMap {
    fn new(pages: &'static mut [PageTable]) -> Self {
        Self {
            mapping: Mapping::new(
                IdTranslation {
                    unused_pages: pages,
                },
                0,
                ROOT_LEVEL,
                TranslationRegime::El3,
                VaRange::Lower,
            ),
        }
    }

    fn map_range(&mut self, range: &MemoryRegion, flags: Attributes) -> Result<(), MapError> {
        let pa = IdTranslation::virtual_to_physical(range.start());
        self.mapping
            .map_range(range, pa, flags, Constraints::empty())
    }

    fn mark_active(&mut self) {
        self.mapping.mark_active(0);
    }

    fn root_address(&self) -> PhysicalAddress {
        self.mapping.root_address()
    }
}
