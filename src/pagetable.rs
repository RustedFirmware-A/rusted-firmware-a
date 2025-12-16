// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod early_pagetable;

use crate::{
    aarch64::{dsb_sy, isb, tlbi_alle3},
    layout::{
        bl_code_base, bl_code_end, bl_ro_data_base, bl_ro_data_end, bl31_end, bl31_start, bss2_end,
        bss2_start,
    },
    platform::{Platform, PlatformImpl},
};
use aarch64_paging::{
    Mapping,
    descriptor::{Attributes, PhysicalAddress, VirtualAddress},
    mair::{Mair, MairAttribute, NormalMemory},
    paging::{Constraints, MemoryRegion, PageTable, Translation, TranslationRegime, VaRange},
};
use arm_sysregs::{SctlrEl3, read_sctlr_el3, write_sctlr_el3, write_ttbr0_el3};
use core::{
    fmt::{self, Debug, Formatter},
    ptr::NonNull,
};
use log::{debug, info, trace};
use spin::{
    Once,
    mutex::{SpinMutex, SpinMutexGuard},
};

const ROOT_LEVEL: usize = 1;

// Indices of entries in the Memory Attribute Indirection Register.
const MAIR_IWTRWA_OWTRWA_NTR_INDEX: u8 = 0;
const MAIR_DEVICE_INDEX: u8 = 1;
const MAIR_NON_CACHEABLE_INDEX: u8 = 2;

// Values for MAIR entries.
const MAIR_DEVICE: MairAttribute = MairAttribute::DEVICE_NGNRE;

// Set write-through mode to ensure all written values are propagated to system memory.
// This guarantees correct Once and Mutex behavior before enabling the MMU.
const MAIR_IWTRWA_OWTRWA_NTR: MairAttribute = MairAttribute::normal(
    NormalMemory::WriteThroughTransientReadWriteAllocate,
    NormalMemory::WriteThroughTransientReadWriteAllocate,
);
const MAIR_NON_CACHEABLE: MairAttribute =
    MairAttribute::normal(NormalMemory::NonCacheable, NormalMemory::NonCacheable);

#[cfg_attr(test, allow(unused))]
const MAIR: Mair = Mair::EMPTY
    .with_attribute(MAIR_DEVICE_INDEX, MAIR_DEVICE)
    .with_attribute(MAIR_IWTRWA_OWTRWA_NTR_INDEX, MAIR_IWTRWA_OWTRWA_NTR)
    .with_attribute(MAIR_NON_CACHEABLE_INDEX, MAIR_NON_CACHEABLE);

#[cfg_attr(test, allow(unused))]
const TCR: u64 = (0b101 << 16) // 48 bit physical address size (256 TiB).
        | (64 - 39); // Size offset is 2**39 bytes (512 GiB).

const TOP_LEVEL_BLOCK_SIZE: usize = 0x4000_0000; // 1GB block size at level 0
const TOP_LEVEL_DESCRIPTOR_COUNT: usize = 512; // 512 descriptors in the level 0 table.
pub const GRANULE_SIZE: usize = 4096; // Using 4k pages.

// Attribute values corresponding to the above MAIR indices.
const IWTRWA_OWTRWA_NTR: Attributes = Attributes::ATTRIBUTE_INDEX_0;
const DEVICE: Attributes = Attributes::ATTRIBUTE_INDEX_1;
const NON_CACHEABLE: Attributes = Attributes::ATTRIBUTE_INDEX_2;

/// Attribute bits which are RES1 for the EL3 translation regime, as we configure it.
///
/// From Arm ARM K.a, D8.3.1.2 Fig. D8-16: lower attributes AP\[1\] bit 6
/// and D8.4.1.2.1 Stage 1 data accesses using Direct permissions:
/// "For a stage 1 translation that supports one Exception level, AP\[1\] is RES1."
const EL3_RES1: Attributes = Attributes::USER;

/// Attribute bit for NSE aka Root state for FEAT_RME
///
/// From ARM DDI 0487K.a, D8-49 Stage 1 VMSAv8-64 Block and Page descriptor fields,
/// the NSE bit is aliased with the Not-global (nG) flag (bit 11).
const NSE: Attributes = Attributes::NON_GLOBAL;

/// Attributes used for all mappings.
///
/// We always set the access flag, as we don't manage access flag faults.
const BASE: Attributes = if cfg!(feature = "rme") {
    EL3_RES1
        .union(Attributes::ACCESSED)
        .union(Attributes::VALID)
        .union(NSE)
} else {
    EL3_RES1
        .union(Attributes::ACCESSED)
        .union(Attributes::VALID)
};

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
pub const MT_MEMORY: Attributes = IWTRWA_OWTRWA_NTR
    .union(BASE)
    .union(Attributes::INNER_SHAREABLE);

/// Attributes used for code (i.e. text) mappings.
pub const MT_CODE: Attributes = {
    let attrs = MT_MEMORY.union(Attributes::READ_ONLY);
    if cfg!(bti) {
        attrs.union(Attributes::GP)
    } else {
        attrs
    }
};

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

/// The runtime page table address is shared via this variable. After the primary core finished
/// initializing the page tables it sets this value to point to the address of the top level table.
/// The variable is written when primary core uses the early page tables, so flushing the variable
/// from the cache to the system memory is required. This is the only time when the variable is
/// modified.
/// The secondary core early boot sequence reads the variable with device attributes and then uses
/// it set its TTBR.
pub static mut PAGE_TABLE_ADDR: usize = 0;

/// Initialises and enables the runtime page tables.
///
/// At this point the early page tables are active with the required MAIR and TCR values, so the
/// function only switches the TTBR value and updates SCTLR to add WXN.
///
/// This should be called once in the startup sequence of the primary core.
pub fn init_runtime_mapping() {
    PAGE_TABLE.call_once(|| {
        let page_heap =
            SpinMutexGuard::leak(PAGE_HEAP.try_lock().expect("Page heap was already taken"));
        let mut idmap = init_page_table(page_heap);

        trace!("Page table: {idmap:?}");

        // Safety: `PAGE_TABLE_ADDR` is only written once here and then its value is flushed from
        // the cache to make it visible to other core's early boot sequence.
        unsafe {
            // Expose page table address so other cores can access it in their boot sequence.
            PAGE_TABLE_ADDR = idmap.root_address().0;

            #[cfg(all(target_arch = "aarch64", not(test)))]
            asm::flush_dcache_range(&raw mut PAGE_TABLE_ADDR as usize, size_of::<usize>());
        }

        info!("Setting MMU config");

        let mut sctlr = read_sctlr_el3();
        assert!(sctlr.contains(SctlrEl3::C | SctlrEl3::M));

        // Ensure all translation table writes have drained into memory.
        dsb_sy();
        isb();

        // Safety: The MMU is already enabled with the correct configuration parameters MAIR, TCR.
        // `idmap` provides a valid address to the runtime page tables.
        unsafe {
            write_ttbr0_el3(idmap.root_address().0);
        }

        // Make sure that any entry from the early page table is invalidated. If WXN is not cached,
        // setting WXN removes execution right of the current PC that would result in an exception.
        tlbi_alle3();
        isb();

        sctlr |= SctlrEl3::WXN;

        // Safety: `sctlr` is a valid and safe value for the EL3 system control register. At this
        // point we only set `WXN` to prevent having RWX regions.
        unsafe {
            write_sctlr_el3(sctlr);
        }
        isb();

        // Invalidate entries to prevent having entries cached without WXN.
        tlbi_alle3();
        isb();

        info!("Marking page table as active");
        idmap.mark_active();

        SpinMutex::new(idmap)
    });
}

/// Enables the MMU for a newly booted core.
///
/// Sets `MAIR_EL3`, `TCR_EL3` and `TTBR0_EL3` then sets
/// `SCTLR_EL3 = (SCTLR_EL3 | sctlr_set) & !sctlr_clear`.
#[cfg(all(target_arch = "aarch64", not(test)))]
#[unsafe(naked)]
pub extern "C" fn enable_mmu(ttbr: usize, sctlr_set: u64, sctlr_clear: u64) {
    crate::naked_asm!(
        "tlbi	alle3

        ldr	x3, ={mair}
        msr	mair_el3, x3

        ldr	x3, ={tcr}
        msr	tcr_el3, x3

        msr	ttbr0_el3, x0

        dsb	sy
        isb

        mrs	x3, sctlr_el3

        orr	x3, x3, x1
        bic	x3, x3, x2

        msr	sctlr_el3, x3

        isb
        ret",
        mair = const MAIR.0,
        tcr = const TCR,
    )
}

/// Creates the page table and maps initial regions needed for boot, including any platform-specific
/// regions.
fn init_page_table(pages: &'static mut [PageTable]) -> IdMap {
    let mut idmap = IdMap::new(pages);

    // If the BL32 entry point is in the middle of our memory range then something is misconfigured.
    let secure_entry_pc = PlatformImpl::secure_entry_point().pc;
    assert!(secure_entry_pc < bl31_start() || secure_entry_pc >= bl31_end());
    assert!(secure_entry_pc < bss2_start() || secure_entry_pc >= bss2_end());

    // SAFETY: Nothing is being unmapped, and the regions being mapped have the correct attributes.
    unsafe {
        // Corresponds to `bl_regions` in C TF-A, `plat/arm/common/arm_bl31_setup.c`.
        // BL31_TOTAL
        idmap.map_region(&MemoryRegion::new(bl31_start(), bl31_end()), MT_MEMORY);
        // BL31_RO
        idmap.map_region(&MemoryRegion::new(bl_code_base(), bl_code_end()), MT_CODE);
        idmap.map_region(
            &MemoryRegion::new(bl_ro_data_base(), bl_ro_data_end()),
            MT_RO_DATA,
        );
        let bss2_start = bss2_start();
        let bss2_end = bss2_end();
        if bss2_start != bss2_end {
            idmap.map_region(&MemoryRegion::new(bss2_start, bss2_end), MT_RW_DATA);
        }
    }

    // Corresponds to `plat_regions` in C TF-A.
    PlatformImpl::map_extra_regions(&mut idmap);

    idmap
}

/// # Safety
///
/// Caller must guarantee that it is safe to disable the MMU at the time of calling this function.
#[allow(unused)]
pub unsafe fn disable_mmu_el3() {
    let mut sctlr_el3 = read_sctlr_el3();
    sctlr_el3.remove(SctlrEl3::C | SctlrEl3::M);
    // SAFETY: `sctlr` is a valid and safe value for the EL3 system control register. Caller
    // promises that we can safely disable the MMU.
    unsafe {
        write_sctlr_el3(sctlr_el3);
    }
    isb();
    dsb_sy();
}

struct IdTranslation {
    /// Pages which can be allocated for page tables.
    pages: &'static mut [PageTable],
    /// Record of which `pages` are currently allocated.
    allocated: [bool; PlatformImpl::PAGE_HEAP_PAGE_COUNT],
}

impl Debug for IdTranslation {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("IdTranslation")
            .field("pages", &self.pages.len())
            .field("allocated", &self.allocated)
            .finish()
    }
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
        let index = self
            .allocated
            .iter()
            .position(|&allocated| !allocated)
            .expect("Failed to allocate page table");
        self.allocated[index] = true;
        let table = NonNull::from(&mut self.pages[index]);
        (
            table,
            Self::virtual_to_physical(VirtualAddress(table.as_ptr() as usize)),
        )
    }

    unsafe fn deallocate_table(&mut self, page_table: NonNull<PageTable>) {
        let index =
            (page_table.addr().get() - &raw const self.pages[0] as usize) / size_of::<PageTable>();
        self.allocated[index] = false;
    }

    fn physical_to_virtual(&self, page_table_pa: PhysicalAddress) -> NonNull<PageTable> {
        NonNull::new(page_table_pa.0 as *mut PageTable)
            .expect("Got physical address 0 for pagetable")
    }
}

#[derive(Debug)]
pub struct IdMap {
    mapping: Mapping<IdTranslation>,
}

impl IdMap {
    fn new(pages: &'static mut [PageTable]) -> Self {
        Self {
            mapping: Mapping::new(
                IdTranslation {
                    pages,
                    allocated: [false; PlatformImpl::PAGE_HEAP_PAGE_COUNT],
                },
                0,
                ROOT_LEVEL,
                TranslationRegime::El3,
                VaRange::Lower,
            ),
        }
    }

    fn mark_active(&mut self) {
        self.mapping.mark_active();
    }

    fn root_address(&self) -> PhysicalAddress {
        self.mapping.root_address()
    }

    /// Adds the given region to the page table with the given attributes, logging it first.
    ///
    /// # Safety
    ///
    /// Memory which is still used by RF-A must not be unmapped, or mapped with incorrect
    /// attributes.
    pub unsafe fn map_region(&mut self, region: &MemoryRegion, attributes: Attributes) {
        debug!("Mapping {region} as {attributes:?}.");
        assert!(attributes.contains(Attributes::VALID));
        let pa = IdTranslation::virtual_to_physical(region.start());
        self.mapping
            .map_range(region, pa, attributes, Constraints::empty())
            .expect("Error mapping memory range");
    }

    /// Unmaps the given memory regions from the page table, and removes any subtables which are no
    /// longer needed as a result.
    ///
    /// # Safety
    ///
    /// The regions being unmapped must not include any memory which is still used by RF-A after
    /// this point.
    #[allow(unused)]
    pub unsafe fn unmap_regions(&mut self, regions: &[MemoryRegion]) {
        for region in regions {
            debug!("Unmapping {region}.");
            self.mapping
                .map_range(
                    region,
                    PhysicalAddress(0),
                    Attributes::empty(),
                    Constraints::empty(),
                )
                .expect("Error unmapping memory range");
        }
        self.mapping.compact_subtables();
    }
}

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use crate::debug::DEBUG;
    use core::arch::global_asm;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("cache_helpers.S"),
        include_str!("asm_macros_common_purge.S"),
        DEBUG = const DEBUG as i32,
    );

    unsafe extern "C" {
        pub fn flush_dcache_range(addr: usize, size: usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_page_table() {
        assert_ne!(PlatformImpl::PAGE_HEAP_PAGE_COUNT, 0);

        let page_heap =
            SpinMutexGuard::leak(PAGE_HEAP.try_lock().expect("Page heap was already taken"));

        let mut idmap = init_page_table(page_heap);
        assert_ne!(idmap.root_address().0, 0);
        idmap.mark_active();
        // `aarch64-paging` will detect the dropped idmap and panic
        core::mem::forget(idmap);
    }
}
