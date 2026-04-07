// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

// TODO: Temporary until the RME feature is fully implemented.
#![allow(unused, dead_code)]

mod aarch64;
mod table;

use crate::{
    aarch64::{TlbiSize, dsb_osh, dsb_oshst, tlbi_rpalos},
    gpt::table::{
        ContigSize, ContiguousDescriptorRef, Level0DescriptorRef, Level0Table, Level1Descriptor,
        Level1DescriptorRef, Level1DescriptorRefMut, Level1Table,
    },
};
use arm_sysregs::{
    el1::accessors::{read_id_aa64mmfr4_el1, read_id_aa64pfr0_el1},
    el3::{accessors::read_gpccr_el3, registers::GpccrEl3},
};
use core::fmt::Debug;
use num_enum::{IntoPrimitive, TryFromPrimitive};
pub use table::GPIAccessType;

/// Generates a bitmask:
/// - `mask!(end, start)`: bits from `start` (inclusive) to `end` (exclusive) are set to 1.
/// - `mask!(len)`: bits from 0 to `len` (exclusive)  are set to 1.
macro_rules! mask {
    ($end:tt, $start:tt) => {
        (mask!($end) & !mask!($start))
    };
    (64) => {
        // Avoid arithmetic overflow when generating a mask of length 64.
        0xFFFF_FFFF_FFFF_FFFF
    };
    ($len:expr) => {
        ((1 << $len) - 1)
    };
}
pub(crate) use mask;

pub type PA = usize;

/// Errors returned when manipulating the [`GranuleProtection`] object.
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    GptNotInitialized,
    InvalidConfiguration,
    MisalignedL0Buffer,
}

/// Errors returned when manipulating the [`GPIAccessType`] mappings in the
/// [`GranuleProtection`] object.
#[derive(Debug, PartialEq, Eq)]
pub enum GranuleError {
    InvalidRequest,
    InvalidL0Entry,
    InvalidL1Entry,
}

/// Handle to manipulate the Granule Protection Table and related registers.
pub struct GranuleProtection<'a> {
    level0: Level0Table<'a>,
    config: GranuleProtectionConfig,
}

impl<'a> Debug for GranuleProtection<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GranuleProtection")
            .field("level0", &self.level0.0.as_ptr())
            .field("config", &self.config)
            .finish()
    }
}

impl<'a> GranuleProtection<'a> {
    /// `PhysicalGranuleSize` used by the `GranuleProtection` in bytes.
    pub fn pgs(&self) -> usize {
        self.config.pgs.size()
    }

    /// Checks if the `GPIAccessType` value is supported on the current system based on available
    /// features and bits of `GpccrEl3`.
    pub fn is_gpi_supported(&self, gpi: GPIAccessType) -> bool {
        // TODO: cache these values in `GranuleProtectionConfig`.
        let gpccr_el3 = read_gpccr_el3();
        match gpi {
            GPIAccessType::SystemAgent => {
                gpccr_el3.contains(GpccrEl3::SA)
                    && read_id_aa64mmfr4_el1().is_feat_rme_gdi_present()
            }
            GPIAccessType::NonSecureProtected => {
                gpccr_el3.contains(GpccrEl3::NSP)
                    && read_id_aa64mmfr4_el1().is_feat_rme_gdi_present()
            }
            GPIAccessType::NoAccess6 => {
                gpccr_el3.contains(GpccrEl3::NA6)
                    && read_id_aa64mmfr4_el1().is_feat_rme_gdi_present()
            }
            GPIAccessType::NoAccess7 => {
                gpccr_el3.contains(GpccrEl3::NA7)
                    && read_id_aa64mmfr4_el1().is_feat_rme_gdi_present()
            }
            GPIAccessType::NonSecureOnly => {
                gpccr_el3.contains(GpccrEl3::NSO)
                    && read_id_aa64pfr0_el1().is_feat_rme_gpc2_present()
            }
            _ => true,
        }
    }

    /// Updates an access control mapping in the GPT.
    ///
    /// - `base_pa`: Address of the granule whose GPI is updated.
    /// - `gpi`: Describes which Physical Address Space the granule will belong to.
    pub fn set(&mut self, base_pa: PA, gpi: GPIAccessType) -> Result<(), GranuleError> {
        if base_pa >= self.config.pps.size() {
            return Err(GranuleError::InvalidRequest);
        }
        // Do not allow setting to a currently unsupported GPI value.
        if !self.is_gpi_supported(gpi) {
            return Err(GranuleError::InvalidRequest);
        }
        let l0_idx = self.config.l0_resolve(base_pa);
        let l0_entry = &self.level0.0[l0_idx];

        let mut l0_entry = match l0_entry.try_into() {
            Ok(Level0DescriptorRef::Table(table)) => table,
            // We do not support changing the GPI of an L0 block descriptor.
            Ok(Level0DescriptorRef::Block(_)) => return Err(GranuleError::InvalidRequest),
            // Not block or table descriptor
            Err(_) => return Err(GranuleError::InvalidL0Entry),
        };

        // Safety:
        // - l0_entry is an entry from the GPT, and the GPT is assumed to be programmed correctly.
        // - self.config is the config of the GranuleProtection object.
        let l1_table: &mut [Level1Descriptor] = unsafe { l0_entry.to_table_mut(&self.config) };
        let l1_idx = self.config.l1_resolve(base_pa);
        let l1_desc = l1_table[l1_idx];
        if let Ok(Level1DescriptorRef::Contiguous(contig)) = (&l1_desc).try_into() {
            self.shatter_contig(base_pa, contig.size(), contig.gpi(), l1_table);
            dsb_oshst();

            // Ensure that all agents observe the new configuration.
            tlbi_rpalos(base_pa, contig.size().into());
            dsb_osh();
        }

        // Either was originally a Granule, or created at the shattering step above.
        if let Ok(Level1DescriptorRefMut::Granule(mut granule)) = (&mut l1_table[l1_idx]).try_into()
        {
            let gran_idx = self.config.granule_resolve(base_pa);
            granule.set_gpi(gran_idx, gpi);
            dsb_oshst();

            // Ensure that all agents observe the new configuration.
            tlbi_rpalos(base_pa, self.config.pgs.into());
            dsb_osh();

            self.fuse_descriptors(base_pa, gpi, l1_table)
        } else {
            // Shattering a valid contiguous descriptor above always creates a granule descriptor.
            Err(GranuleError::InvalidL1Entry)
        }
    }

    /// Looks up the access control mapping of the memory region starting at `base_pa` from the GPT.
    pub fn lookup(&self, base_pa: PA) -> Result<GPIAccessType, GranuleError> {
        if base_pa >= self.config.pps.size() {
            return Err(GranuleError::InvalidRequest);
        }
        let l0_idx = self.config.l0_resolve(base_pa);
        let l0_entry = &self.level0.0[l0_idx];

        let l0_entry = match l0_entry.try_into() {
            Ok(Level0DescriptorRef::Table(table)) => table,
            Ok(Level0DescriptorRef::Block(block)) => {
                let gpi = block.gpi();
                return if self.is_gpi_supported(gpi) {
                    Ok(gpi)
                } else {
                    Err(GranuleError::InvalidL0Entry)
                };
            }
            Err(_) => return Err(GranuleError::InvalidL0Entry),
        };

        // Safety:
        // - l0_entry is an entry from the GPT, and the GPT is assumed to be programmed correctly.
        // - self.config is the config of the GranuleProtection object.
        let l1_table: &[Level1Descriptor] = unsafe { l0_entry.to_table(&self.config) };
        let l1_idx = self.config.l1_resolve(base_pa);
        let l1_desc = &l1_table[l1_idx];

        match l1_desc.try_into() {
            Ok(Level1DescriptorRef::Granule(granule)) => {
                let gran_idx = self.config.granule_resolve(base_pa);
                granule
                    .gpi(gran_idx)
                    .filter(|gpi| self.is_gpi_supported(*gpi))
                    .ok_or(GranuleError::InvalidL1Entry)
            }
            Ok(Level1DescriptorRef::Contiguous(contig)) => {
                let gpi = contig.gpi();
                if self.is_gpi_supported(gpi) {
                    Ok(gpi)
                } else {
                    Err(GranuleError::InvalidL1Entry)
                }
            }
            Err(_) => Err(GranuleError::InvalidL1Entry),
        }
    }

    /// Align `base_pa` to `contig`, and fill the next `ContigSize` range with contiguous descriptors.
    fn fuse(&self, base_pa: PA, gpi: GPIAccessType, l1: &mut Level1Table, contig: ContigSize) {
        let base_aligned = contig.align_pa(base_pa);
        let contig_desc = Level1Descriptor::contig(contig, gpi);
        self.fill_descs(base_aligned, contig.size() / self.pgs(), contig_desc, l1);
    }

    fn fuse_descriptors(
        &self,
        base_pa: PA,
        gpi: GPIAccessType,
        l1: &mut Level1Table,
    ) -> Result<(), GranuleError> {
        // Fuse to the largest possible ContigSize.
        if self.can_fuse(base_pa, gpi, l1, ContigSize::MB2)? {
            if self.can_fuse(base_pa, gpi, l1, ContigSize::MB32)? {
                if self.can_fuse(base_pa, gpi, l1, ContigSize::MB512)? {
                    self.fuse(base_pa, gpi, l1, ContigSize::MB512);
                    return Ok(());
                }
                self.fuse(base_pa, gpi, l1, ContigSize::MB32);
                return Ok(());
            }
            self.fuse(base_pa, gpi, l1, ContigSize::MB2);
            return Ok(());
        }
        Ok(())
    }

    fn can_fuse(
        &self,
        base_pa: PA,
        expected_gpi: GPIAccessType,
        l1: &Level1Table,
        contig: ContigSize,
    ) -> Result<bool, GranuleError> {
        let base_aligned = contig.align_pa(base_pa);
        let step = 16 * self.pgs();
        for offset in (0..contig.size()).step_by(step) {
            let pa = base_aligned + offset;

            // Aligning the address to any ContigSize means it still must belong to the same L1 table.
            let l1_desc = l1[self.config.l1_resolve(pa)];
            // After shattering at `base_pa`, this range is described by granules or contiguous
            // descriptors smaller than `contig`, so only the GPI needs to be checked.
            if !l1_desc.matches_gpi(expected_gpi)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Write `desc` in the `l1` GPT table `count` times consecutively.
    ///
    /// Assumptions:
    /// - bits of `base_pa` selecting the L1 table are not checked, it is assumed to point to `l1`,
    fn fill_descs(&self, base_pa: PA, count: usize, desc: Level1Descriptor, l1: &mut Level1Table) {
        let end_pa = base_pa + count * self.pgs();
        for pa in (base_pa..end_pa).step_by(16 * self.pgs()) {
            let l1_idx = self.config.l1_resolve(pa);
            l1[l1_idx] = desc;
        }
    }

    /// Used in `set()` for shattering a contiguous descriptor.
    ///
    /// `base_pa`: address written to in `set()`.
    fn shatter_contig(
        &self,
        base_pa: PA,
        size: ContigSize,
        gpi: GPIAccessType,
        l1: &mut Level1Table,
    ) {
        let base_aligned = size.align_pa(base_pa);
        let Some(smaller) = size.next_smaller() else {
            // Shattering a 2 MB contiguous descriptor to granules.
            let desc = Level1Descriptor::granule(&[gpi; 16]);
            self.fill_descs(base_aligned, size.size() / self.pgs(), desc, l1);
            return;
        };

        let base_small_aligned = smaller.align_pa(base_pa);
        // Fill with the smaller ContigSize variant
        let desc = Level1Descriptor::contig(smaller, gpi);
        // 1. fill before
        let bytes_pre = base_small_aligned - base_aligned;
        self.fill_descs(base_aligned, bytes_pre / self.pgs(), desc, l1);
        // 2. shatter the middle part
        self.shatter_contig(base_pa, smaller, gpi, l1);
        // 3. fill the remaining parts
        let bytes_post = size.size() - smaller.size() - bytes_pre;
        self.fill_descs(
            base_small_aligned + smaller.size(),
            bytes_post / self.pgs(),
            desc,
            l1,
        );
    }
}

/// Protected Physical Address Size.
///
/// The size of the memory region protected by GPTBR_EL3, in terms of the number of
/// least-significant address bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(crate) enum ProtectedPhysicalAddressSize {
    /// Protected addresses space is 4GB.
    GB4 = 0b000,
    /// Protected addresses space is 64GB.
    GB64 = 0b001,
    /// Protected addresses space is 1TB.
    TB1 = 0b010,
    /// Protected addresses space is 4TB.
    TB4 = 0b011,
    /// Protected addresses space is 16TB.
    TB16 = 0b100,
    /// Protected addresses space is 256TB.
    TB256 = 0b101,
    /// Protected addresses space is 4PB.
    PB4 = 0b110,
}

impl ProtectedPhysicalAddressSize {
    /// Returns the corresponding address width.
    pub fn width(&self) -> usize {
        match self {
            Self::GB4 => 32,
            Self::GB64 => 36,
            Self::TB1 => 40,
            Self::TB4 => 42,
            Self::TB16 => 44,
            Self::TB256 => 48,
            Self::PB4 => 52,
        }
    }

    /// Protected Physical Address Size in bytes.
    pub fn size(&self) -> usize {
        0x1 << self.width()
    }
}

/// Number of least-significant address bits protected by each entry in the level 0 GPT.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub(crate) enum Level0GptSize {
    /// L0 entries cover 1GB.
    GB1 = 0b0000,
    /// L0 entries cover 16GB.
    GB16 = 0b0100,
    /// L0 entries cover 64GB.
    GB64 = 0b0110,
    /// L0 entries cover 512GB.
    GB512 = 0b1001,
}

impl Level0GptSize {
    /// Returns the corresponding address width.
    pub fn width(&self) -> usize {
        match self {
            Self::GB1 => 30,
            Self::GB16 => 34,
            Self::GB64 => 36,
            Self::GB512 => 39,
        }
    }
}

/// Physical Granule size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum PhysicalGranuleSize {
    /// Physical granules cover 4KB.
    KB4 = 0b00,
    /// Physical granules cover 64KB.
    KB64 = 0b01,
    /// Physical granules cover 16KB.
    KB16 = 0b10,
}

impl PhysicalGranuleSize {
    /// Returns the corresponding address width.
    pub fn width(&self) -> usize {
        match self {
            Self::KB4 => 12,
            Self::KB16 => 14,
            Self::KB64 => 16,
        }
    }

    /// Physical Granule Size in bytes.
    pub fn size(&self) -> usize {
        0x1 << self.width()
    }
}

impl From<PhysicalGranuleSize> for TlbiSize {
    fn from(size: PhysicalGranuleSize) -> Self {
        match size {
            PhysicalGranuleSize::KB4 => Self::KB4,
            PhysicalGranuleSize::KB16 => Self::KB16,
            PhysicalGranuleSize::KB64 => Self::KB64,
        }
    }
}

impl From<ContigSize> for TlbiSize {
    fn from(size: ContigSize) -> Self {
        match size {
            ContigSize::MB2 => Self::MB2,
            ContigSize::MB32 => Self::MB32,
            ContigSize::MB512 => Self::MB512,
        }
    }
}

/// Size configuration of the [`GranuleProtection`] object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GranuleProtectionConfig {
    /// [`ProtectedPhysicalAddressSize`] used by this [`GranuleProtection`].
    pps: ProtectedPhysicalAddressSize,
    /// [`Level0GptSize`] used by this [`GranuleProtection`].
    l0gptsz: Level0GptSize,
    /// [`PhysicalGranuleSize`] used by this [`GranuleProtection`].
    pgs: PhysicalGranuleSize,
}

impl GranuleProtectionConfig {
    /// Retrieve the index of the L0 entry referencing the given PA.
    fn l0_resolve(&self, pa: PA) -> usize {
        (pa & mask!(self.pps.width())) >> (self.l0gptsz.width())
    }

    /// Retrieve the index of the L1 entry referencing the given PA.
    fn l1_resolve(&self, pa: PA) -> usize {
        (pa & mask!(self.l0gptsz.width())) >> (self.pgs.width() + 4)
    }

    /// Retrieve the index inside a granule referencing the given PA.
    fn granule_resolve(&self, pa: PA) -> usize {
        (pa >> self.pgs.width()) & 0xF
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use arm_sysregs::{
        el1::{
            fake::SYSREGS as EL1_SYSREGS,
            registers::{IdAa64mmfr4El1, IdAa64pfr0El1},
        },
        el3::{
            fake::SYSREGS as EL3_SYSREGS,
            registers::{GpccrEl3, GptbrEl3},
        },
    };
    use table::Level0Descriptor;

    #[test]
    fn gpc_resolve() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB4,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB4,
        };
        assert_eq!(gpc.l0_resolve(0xabcd_f432_9876), 0x3);
        assert_eq!(gpc.l0_resolve(0xabcd_5432_9876), 0x1);
        assert_eq!(gpc.l1_resolve(0xf432_abcd), 0x3432);
        assert_eq!(gpc.l1_resolve(0xabcd_1234_9876), 0x1234);
        assert_eq!(gpc.granule_resolve(0xabcd_1234_9876), 0x9);
        assert_eq!(gpc.granule_resolve(0xf432_abcd), 0xa);
    }

    #[test]
    fn gpc_resolve_2() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::TB1,
            l0gptsz: Level0GptSize::GB16,
            pgs: PhysicalGranuleSize::KB64,
        };
        assert_eq!(gpc.l0_resolve(0xabcd_f432_9876), 0x33);
        assert_eq!(gpc.l0_resolve(0xcdab_5432_9876), 0x2a);
        assert_eq!(gpc.l1_resolve(0xf432_abcd), 0xf43);
        assert_eq!(gpc.l1_resolve(0xabcd_1234_9876), 0x1123);
        assert_eq!(gpc.granule_resolve(0xabcd_1234_9876), 0x4);
        assert_eq!(gpc.granule_resolve(0xf432_abcd), 0x2);
    }

    /// Dynamically allocates a 'static buffer for `elems` entries of `size` bytes. The resulting
    /// buffer is aligned on `size`.
    fn align(slice: &mut [u8], size: usize, elems: usize) -> &mut [u8] {
        let ptr = slice.as_ptr() as usize;
        let start_ptr = (ptr & !(size - 1)) + size;
        let start = start_ptr - ptr;

        &mut slice[start..start + size * elems]
    }

    use std::cmp::max;
    macro_rules! declare_l0 {
        ($name:ident, $PPS:expr, $L0GPTSZ:expr) => {
            let alignment = max(8 << ($PPS - $L0GPTSZ), 1 << 12);
            let mut $name = vec![0; (alignment) * 2];
            let mut $name = align(&mut $name, alignment, 1);
        };
    }

    macro_rules! declare_l1 {
        ($name:ident, $PGS:expr, $L0GPTSZ:expr, $l1_size:expr) => {
            let mut $name =
                vec![0; (8 << ($L0GPTSZ.width() - ($PGS.width() + 4))) * ($l1_size + 1)];
            let mut $name = align(
                &mut $name,
                8 << ($L0GPTSZ.width() - ($PGS.width() + 4)),
                $l1_size,
            );
        };
    }

    macro_rules! declare_empty_gpt {
        ($name:ident, $l0name:ident, $GPC:expr) => {
            declare_l0!($l0name, $GPC.pps.width(), $GPC.l0gptsz.width());
            let base = $l0name.as_ptr() as u64;
            assert_eq!(base & mask!(12), 0, "base is not aligned to 4KB");

            let mut gptbr = GptbrEl3::empty();
            gptbr.set_baddr(base >> 12);
            EL3_SYSREGS.lock().unwrap().gptbr_el3 = gptbr;
            let gpccr = GpccrEl3::GPC
            .with_pps($GPC.pps as u8)
            .with_l0gptsz($GPC.l0gptsz as u8)
            .with_pgs($GPC.pgs as u8);
            EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
            let mut $name =
                // SAFETY: Each test only calls this once.
                unsafe { GranuleProtection::discover().expect("failed to discover GPT") };
	        assert_eq!($name.config, $GPC);
        };
    }

    /// Declare a GPT and initialize all L0 entries as NoAccess blocks.
    macro_rules! declare_gpt_noaccess {
        ($name:ident, $l0name:ident, $GPC:expr) => {
            declare_empty_gpt!($name, $l0name, $GPC);
            let desc = Level0Descriptor::block(GPIAccessType::NoAccess);
            let bytes = desc.as_bytes();
            for chunk in $l0name.chunks_exact_mut(core::mem::size_of::<Level0Descriptor>()) {
                chunk.copy_from_slice(bytes);
            }
        };
    }

    /// Change an L0 entry to a TableDescriptor, and point it to an L1 table.
    macro_rules! add_table_at_idx {
        ($gpt:ident, $l1name:ident, $idx:expr) => {
            declare_l1!($l1name, $gpt.config.pgs, $gpt.config.l0gptsz, 1);
            $gpt.level0.0[$idx] = Level0Descriptor::table($l1name.as_ptr() as u64);
        };
    }

    macro_rules! write_block {
        ($l0name:ident, $IDX:literal, $GPI:expr) => {
            let desc = Level0Descriptor::block($GPI);
            let offset = $IDX * size_of::<Level0Descriptor>();
            let bytes = desc.as_bytes();
            $l0name[offset..offset + bytes.len()].copy_from_slice(bytes);
        };
    }

    enum L0Matcher<'a> {
        Block(GPIAccessType),
        Table(&'a [(usize, L1Matcher<'a>)]),
    }

    enum L1Matcher<'a> {
        Contig(ContigSize, GPIAccessType),
        GranulePart(&'a [(usize, GPIAccessType)]),
        Granule([GPIAccessType; 16]),
    }

    use L0Matcher::*;
    use L1Matcher::*;

    /// Checks that the data in `gpt` corresponds to the description given in `expected`. Parts not
    /// described by `expected` must be Block descriptors with [`GPIAccessType`] `NoAccess`.
    fn assert_gpt_eq<'a>(gpt: &GranuleProtection, expected: &'a [(usize, L0Matcher<'a>)]) {
        fn match_l1<'a>(entry: &Level1Descriptor, matcher: &L1Matcher<'a>) {
            match matcher {
                Contig(size, gpi) => {
                    let Ok(Level1DescriptorRef::Contiguous(contig)) = entry.try_into() else {
                        panic!("Did not match contiguous descriptor");
                    };

                    assert_eq!(contig.gpi(), *gpi);
                    assert_eq!(contig.size(), *size);
                }
                GranulePart(items) => {
                    let Ok(Level1DescriptorRef::Granule(granule)) = entry.try_into() else {
                        panic!("Did not match granule descriptor");
                    };

                    for (i, gran) in *items {
                        assert_eq!(granule.gpi(*i).expect("Failed to index granule"), *gran)
                    }
                }
                Granule(items) => {
                    let Ok(Level1DescriptorRef::Granule(granule)) = entry.try_into() else {
                        panic!("Did not match granule descriptor");
                    };

                    for (i, gran) in items.iter().enumerate() {
                        assert_eq!(granule.gpi(i).expect("Failed to index granule"), *gran)
                    }
                }
            }
        }

        fn match_l0<'a>(
            entry: &Level0Descriptor,
            matcher: &L0Matcher<'a>,
            gpc: GranuleProtectionConfig,
        ) {
            match matcher {
                Block(gpi) => {
                    let Ok(Level0DescriptorRef::Block(block)) = entry.try_into() else {
                        panic!("Did not match block descriptor");
                    };

                    assert_eq!(block.gpi(), *gpi)
                }
                Table(items) => {
                    let Ok(Level0DescriptorRef::Table(table_descriptor)) = entry.try_into() else {
                        panic!("Did not match table descriptor");
                    };
                    // Safety: the GPT is assumed to be programmed correctly.
                    let table = unsafe { table_descriptor.to_table(&gpc) };

                    let mut l1_matcher_iter = items.iter().peekable();

                    let mut l1_idx = 0;

                    while l1_idx < table.len() {
                        let l1_entry = &table[l1_idx];

                        if l1_matcher_iter.peek().is_some_and(|(i, _)| *i == l1_idx) {
                            let (_, l1_matcher) = l1_matcher_iter.next().unwrap();

                            match_l1(l1_entry, l1_matcher);
                        } else {
                            match l1_entry.try_into() {
                                Ok(Level1DescriptorRef::Contiguous(contig)) => {
                                    assert_eq!(contig.gpi(), GPIAccessType::NoAccess)
                                }
                                Ok(Level1DescriptorRef::Granule(granule)) => {
                                    assert!(granule.is_empty())
                                }
                                Err(_) => panic!(
                                    "Did not match NoAccess contiguous or empty granule descriptor."
                                ),
                            }
                        }

                        let size = match l1_entry.try_into() {
                            Ok(Level1DescriptorRef::Contiguous(contig)) => {
                                contig.size().size() >> (gpc.pgs.width() + 4)
                            }
                            _ => 1,
                        };

                        l1_idx += size;
                    }
                }
            }
        }

        let mut l0_matcher_iter = expected.iter().peekable();
        for (l0_idx, l0_entry) in gpt.level0.0.iter().enumerate() {
            if l0_matcher_iter.peek().is_some_and(|(i, _)| *i == l0_idx) {
                let (_, l0_matcher) = l0_matcher_iter.next().unwrap();

                match_l0(l0_entry, l0_matcher, gpt.config);
            } else {
                assert!(matches!(
                    l0_entry.try_into(),
                    Ok(Level0DescriptorRef::Block(block))
                        if block.gpi() == GPIAccessType::NoAccess
                ));
            }
        }
    }

    #[test]
    fn gpt_uninit() {
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = GpccrEl3::empty();
        assert_eq!(
            Some(Error::GptNotInitialized),
            // SAFETY: only called once.
            unsafe { GranuleProtection::discover() }.err()
        );
    }

    use core::mem::size_of;
    use zerocopy::IntoBytes;
    #[test]
    fn gpt_get_set() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB4,
        };

        declare_empty_gpt!(gpt, l0table, gpc);

        // Set first descriptor to Block::NoAccess
        write_block!(l0table, 0, GPIAccessType::NoAccess);

        let addr_0 = (1 << gpc.l0gptsz.width()) - 1;
        assert_eq!(gpc.l0_resolve(addr_0), 0);
        assert_eq!(gpt.lookup(addr_0), Ok(GPIAccessType::NoAccess));

        // Create secure block
        let addr_1 = 1 << gpc.l0gptsz.width();
        assert_eq!(gpc.l0_resolve(addr_1), 1);
        write_block!(l0table, 1, GPIAccessType::Secure);
        assert_eq!(gpt.lookup(addr_1), Ok(GPIAccessType::Secure));

        // Create L1 table
        let addr_2 = 2 << gpc.l0gptsz.width();
        declare_l1!(l1table, gpc.pgs, gpc.l0gptsz, 32);
        let base = l1table.as_ptr() as u64;
        let desc = Level0Descriptor::table(base);
        let offset = 2 * size_of::<Level0Descriptor>();
        let bytes = desc.as_bytes();
        l0table[offset..offset + bytes.len()].copy_from_slice(bytes);
        assert_eq!(gpt.lookup(addr_2), Ok(GPIAccessType::NoAccess));

        // Use set() to modify L1 values
        let res = gpt.set(addr_2, GPIAccessType::Secure);
        assert!(res.is_ok());
        assert_eq!(gpt.lookup(addr_2), Ok(GPIAccessType::Secure));

        // Modify other granules in the L1 table
        let gr_0 = addr_2;
        let gr_1 = addr_2 + gpt.pgs();
        let gr_2 = addr_2 + 2 * gpt.pgs();
        let gr_3 = addr_2 + 3 * gpt.pgs();
        let gr_4 = addr_2 + 4 * gpt.pgs();
        assert_eq!(gpt.config.granule_resolve(gr_0), 0);
        assert_eq!(gpt.config.granule_resolve(gr_1), 1);
        assert_eq!(gpt.config.granule_resolve(gr_2), 2);
        assert_eq!(gpt.config.granule_resolve(gr_3), 3);
        assert_eq!(gpt.config.granule_resolve(gr_4), 4);

        assert!(gpt.set(gr_0, GPIAccessType::Realm).is_ok());
        assert!(gpt.set(gr_1, GPIAccessType::Secure).is_ok());
        assert!(gpt.set(gr_2, GPIAccessType::NonSecure).is_ok());
        assert!(gpt.set(gr_3, GPIAccessType::Root).is_ok());

        assert_eq!(gpt.lookup(gr_0), Ok(GPIAccessType::Realm));
        assert_eq!(gpt.lookup(gr_1), Ok(GPIAccessType::Secure));
        assert_eq!(gpt.lookup(gr_2), Ok(GPIAccessType::NonSecure));
        assert_eq!(gpt.lookup(gr_3), Ok(GPIAccessType::Root));
        assert_eq!(gpt.lookup(gr_4), Ok(GPIAccessType::NoAccess));
    }

    #[test]
    fn gpt_invalid_l0() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB4,
        };

        declare_empty_gpt!(gpt, l0table, gpc);

        let addr_0 = (1 << 30) - 1;
        assert_eq!(gpc.l0_resolve(addr_0), 0);

        assert_eq!(gpt.lookup(addr_0), Err(GranuleError::InvalidL0Entry));
    }

    #[test]
    fn gpt_invalid_l1() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB4,
        };

        declare_gpt_noaccess!(gpt, l0table, gpc);
        // Create L1 table
        let addr_1 = 1 << gpc.l0gptsz.width();
        add_table_at_idx!(gpt, l1table, 1);

        // L1 table initialized with zeros means they are granules with NoAccess GPI.
        let gr_0 = addr_1;
        let gr_1 = addr_1 + gpt.pgs();

        assert_eq!(gpt.lookup(gr_0), Ok(GPIAccessType::NoAccess));
        assert_eq!(gpt.lookup(gr_1), Ok(GPIAccessType::NoAccess));

        // Overwrite L1 table with invalid GPI values
        for val in l1table.iter_mut() {
            *val = 0b1110; // invalid
        }

        assert_eq!(gpt.lookup(gr_0), Err(GranuleError::InvalidL1Entry));
    }

    #[test]
    fn gpi_encodings() {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB4,
        };
        declare_empty_gpt!(gpt, l0table, gpc);

        // enable FEAT_RME_GDI
        let mut gpccr = GpccrEl3::empty();
        let mut id_aa64mmfr4_el1 = IdAa64mmfr4El1::empty();
        id_aa64mmfr4_el1.set_rmegdi(0b0001);
        EL1_SYSREGS.lock().unwrap().id_aa64mmfr4_el1 = id_aa64mmfr4_el1;

        // enable FEAT_RME_GPC2
        let mut id_aa64pfr0_el1 = IdAa64pfr0El1::empty();
        id_aa64pfr0_el1.set_rme(0b0010);
        EL1_SYSREGS.lock().unwrap().id_aa64pfr0_el1 = id_aa64pfr0_el1;

        assert_eq!(Err(()), GPIAccessType::try_from(0b1_0000_0000u64));

        let byte = GPIAccessType::SystemAgent as u8;
        let gpi = GPIAccessType::try_from(byte).unwrap();
        assert_eq!(GPIAccessType::SystemAgent, gpi);
        assert!(!gpt.is_gpi_supported(gpi));
        gpccr |= GpccrEl3::SA;
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
        assert!(gpt.is_gpi_supported(gpi));

        let byte = GPIAccessType::NonSecureProtected as u8;
        let gpi = GPIAccessType::try_from(byte).unwrap();
        assert_eq!(GPIAccessType::NonSecureProtected, gpi);
        assert!(!gpt.is_gpi_supported(gpi));
        gpccr |= GpccrEl3::NSP;
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
        assert!(gpt.is_gpi_supported(gpi));

        let byte = GPIAccessType::NoAccess6 as u8;
        let gpi = GPIAccessType::try_from(byte).unwrap();
        assert_eq!(GPIAccessType::NoAccess6, gpi);
        assert!(!gpt.is_gpi_supported(gpi));
        gpccr |= GpccrEl3::NA6;
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
        assert!(gpt.is_gpi_supported(gpi));

        let byte = GPIAccessType::NoAccess7 as u8;
        let gpi = GPIAccessType::try_from(byte).unwrap();
        assert_eq!(GPIAccessType::NoAccess7, gpi);
        assert!(!gpt.is_gpi_supported(gpi));
        gpccr |= GpccrEl3::NA7;
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
        assert!(gpt.is_gpi_supported(gpi));

        let byte = GPIAccessType::NonSecureOnly as u8;
        let gpi = GPIAccessType::try_from(byte).unwrap();
        assert_eq!(GPIAccessType::NonSecureOnly, gpi);
        assert!(!gpt.is_gpi_supported(gpi));
        gpccr |= GpccrEl3::NSO;
        EL3_SYSREGS.lock().unwrap().gpccr_el3 = gpccr;
        assert!(gpt.is_gpi_supported(gpi));
    }

    #[test]
    fn tables() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };

        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 1);
        add_table_at_idx!(gpt, l1, 2);

        assert_gpt_eq(
            &gpt,
            &[
                (0, Block(GPIAccessType::NoAccess)),
                (1, Table(&[(0, Granule([GPIAccessType::NoAccess; 16]))])),
                (2, Table(&[(0, Granule([GPIAccessType::NoAccess; 16]))])),
                (3, Block(GPIAccessType::NoAccess)),
            ],
        );

        Ok(())
    }
    #[test]
    fn set_granules() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };
        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 0);

        gpt.set(0, GPIAccessType::Root)?;
        gpt.set(1 * gpt.pgs(), GPIAccessType::Realm)?;
        gpt.set(2 * gpt.pgs(), GPIAccessType::Secure)?;
        gpt.set(3 * gpt.pgs(), GPIAccessType::NonSecure)?;

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[(
                    0,
                    GranulePart(&[
                        (0, GPIAccessType::Root),
                        (1, GPIAccessType::Realm),
                        (2, GPIAccessType::Secure),
                        (3, GPIAccessType::NonSecure),
                    ]),
                )]),
            )],
        );

        Ok(())
    }
    #[test]
    fn contig_2mb() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };
        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 0);

        for pa in (0..0x20_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Root)?;
        }

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[(0, Contig(ContigSize::MB2, GPIAccessType::Root))]),
            )],
        );

        Ok(())
    }

    #[test]
    fn contigs_2mb() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };
        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 0);

        for pa in (0..0x20_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Root)?;
        }
        for pa in (0x20_0000..0x40_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Realm)?;
        }

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (0, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (2, Contig(ContigSize::MB2, GPIAccessType::Realm)),
                ]),
            )],
        );

        Ok(())
    }

    #[test]
    fn contig_shatter() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };
        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 0);

        for pa in (0..0x20_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Root)?;
        }

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[(0, Contig(ContigSize::MB2, GPIAccessType::Root))]),
            )],
        );

        gpt.set(0x10_0000, GPIAccessType::Secure)?;
        gpt.set(0x10_0000 + 4 * gpt.pgs(), GPIAccessType::Secure)?;

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (0, Granule([GPIAccessType::Root; 16])),
                    (
                        1,
                        GranulePart(&[
                            (0, GPIAccessType::Secure),
                            (1, GPIAccessType::Root),
                            (2, GPIAccessType::Root),
                            (3, GPIAccessType::Root),
                            (4, GPIAccessType::Secure),
                            (5, GPIAccessType::Root),
                            (6, GPIAccessType::Root),
                            (7, GPIAccessType::Root),
                            (8, GPIAccessType::Root),
                            (9, GPIAccessType::Root),
                            (10, GPIAccessType::Root),
                            (11, GPIAccessType::Root),
                            (12, GPIAccessType::Root),
                            (13, GPIAccessType::Root),
                            (14, GPIAccessType::Root),
                            (15, GPIAccessType::Root),
                        ]),
                    ),
                ]),
            )],
        );

        Ok(())
    }

    #[test]
    fn contig_512mb() -> Result<(), GranuleError> {
        let gpc = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::GB64,
            l0gptsz: Level0GptSize::GB1,
            pgs: PhysicalGranuleSize::KB64,
        };

        declare_gpt_noaccess!(gpt, l0, gpc);
        add_table_at_idx!(gpt, l1, 0);

        for pa in (0..0x4000_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Root)?;
        }

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (0, Contig(ContigSize::MB512, GPIAccessType::Root)),
                    (512, Contig(ContigSize::MB512, GPIAccessType::Root)),
                ]),
            )],
        );

        gpt.set(0x0, GPIAccessType::Realm)?;

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (
                        // First MB as granules, with the modified at 0x0.
                        0,
                        GranulePart(&[
                            (0, GPIAccessType::Realm),
                            (1, GPIAccessType::Root),
                            (2, GPIAccessType::Root),
                            (3, GPIAccessType::Root),
                            (4, GPIAccessType::Root),
                            (5, GPIAccessType::Root),
                            (6, GPIAccessType::Root),
                            (7, GPIAccessType::Root),
                            (8, GPIAccessType::Root),
                            (9, GPIAccessType::Root),
                            (10, GPIAccessType::Root),
                            (11, GPIAccessType::Root),
                            (12, GPIAccessType::Root),
                            (13, GPIAccessType::Root),
                            (14, GPIAccessType::Root),
                            (15, GPIAccessType::Root),
                        ]),
                    ),
                    // Second MB of granules
                    (1, Granule([GPIAccessType::Root; 16])),
                    // Rest of the first 32MB as 2MB contigs
                    (1 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (2 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (3 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (4 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (5 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (6 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (7 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (8 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (9 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (10 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (11 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (12 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (13 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (14 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    (15 * 2, Contig(ContigSize::MB2, GPIAccessType::Root)),
                    // Rest of the first 512MB as 32MB contigs
                    (1 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (2 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (3 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (4 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (5 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (6 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (7 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (8 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (9 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (10 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (11 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (12 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (13 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (14 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    (15 * 32, Contig(ContigSize::MB32, GPIAccessType::Root)),
                    // Second 512MB contig unchanged
                    (512, Contig(ContigSize::MB512, GPIAccessType::Root)),
                ]),
            )],
        );

        // Check if changing back to Root triggers fusing back to 512MB contigs
        gpt.set(0x0, GPIAccessType::Root)?;

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (0, Contig(ContigSize::MB512, GPIAccessType::Root)),
                    (512, Contig(ContigSize::MB512, GPIAccessType::Root)),
                ]),
            )],
        );

        // Check that overwriting the whole range with granules works.

        for pa in (0..0x4000_0000).step_by(gpt.pgs()) {
            gpt.set(pa, GPIAccessType::Secure)?;
        }

        assert_gpt_eq(
            &gpt,
            &[(
                0,
                Table(&[
                    (0, Contig(ContigSize::MB512, GPIAccessType::Secure)),
                    (512, Contig(ContigSize::MB512, GPIAccessType::Secure)),
                ]),
            )],
        );

        Ok(())
    }
}
