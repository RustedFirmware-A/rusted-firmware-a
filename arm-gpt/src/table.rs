// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::fmt::Debug;
use core::slice::from_raw_parts_mut;

use num_enum::{IntoPrimitive, TryFromPrimitive};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::{GranuleProtectionConfig, PA, mask};

/// Creates an accessor function for a type wrapping an integer.
///
/// The accessor will extract the value in the specified range and convert into the requested type
/// using [`TryInto`].
macro_rules! declare_accessor {
    ($end:literal : $start:literal, $name:ident, $ty:ty) => {
        #[doc = "Return the [`"]
        #[doc = stringify!($ty)]
        #[doc = "`] of this descriptor."]
        pub fn $name(&self) -> $ty {
            ((self.0.0 >> $start) & mask!($end - $start))
                .try_into()
                .unwrap()
        }
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafDescriptorType {
    Block,
    Granule,
    Contig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
/// Access control restriction that can be applied to a memory region in the GPT.
pub enum GPIAccessType {
    /// No accesses permitted..
    NoAccess = 0b0000,
    /// Accesses permitted to Secure PA space only..
    Secure = 0b1000,
    /// Accesses permitted to Non-secure PA space only.
    NonSecure = 0b1001,
    /// Accesses permitted to Root PA space only.
    Root = 0b1010,
    /// Accesses permitted to Realm PA space only.
    Realm = 0b1011,
    /// All accesses permitted.
    Any = 0b1111,
}

impl GPIAccessType {
    const MASK: u64 = mask!(4);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u64)]
/// Possible sizes of a Contiguous Descriptor.
pub enum ContigSize {
    /// Descriptor covers 2MB of memory.
    MB2 = 0b01,
    /// Descriptor covers 32MB of memory.
    MB32 = 0b10,
    /// Descriptor covers 512MB of memory.
    MB512 = 0b11,
}

impl ContigSize {
    const MASK: u64 = mask!(2);
}

impl ContigSize {
    pub const VALUES: [Self; 3] = [Self::MB2, Self::MB32, Self::MB512];

    pub fn allowed_shifts() -> impl Iterator<Item = usize> {
        Self::VALUES.iter().map(|v| v.shift())
    }

    /// Returns the bitshift corresponding to this size's alignment.
    pub const fn shift(&self) -> usize {
        match self {
            ContigSize::MB2 => 21,
            ContigSize::MB32 => 25,
            ContigSize::MB512 => 29,
        }
    }

    /// Returns the [`ContigSize`] corresponding to the given alignment.
    pub const fn from_shift(shift: usize) -> Option<Self> {
        match shift {
            21 => Some(ContigSize::MB2),
            25 => Some(ContigSize::MB32),
            29 => Some(ContigSize::MB512),
            _ => None,
        }
    }

    /// The size in bytes.
    pub const fn size(&self) -> usize {
        1 << self.shift()
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromBytes, IntoBytes, KnownLayout, Immutable,
)]
#[repr(transparent)]
pub(crate) struct Level0Descriptor(pub(crate) u64);

impl Level0Descriptor {
    const TAG_MASK: u64 = mask!(4);
    const BLOCK_TAG: u64 = 0b0001;
    const TABLE_TAG: u64 = 0b0011;

    const TABLE_ADDR_LEN: usize = 52;
    const TABLE_ADDR_ALIGN: usize = 12;
    const TABLE_ADDR_MASK: usize = mask!((Self::TABLE_ADDR_LEN), (Self::TABLE_ADDR_ALIGN));

    /// Tries to create a view of this Level 0 Descriptor as a Table Descriptor.
    ///
    /// Returns a [`TableDescriptorRef`] referencing `self`, or [`None`] if self does not contain
    /// a Table Descriptor.
    pub const fn as_table<'a>(&'a self) -> Option<TableDescriptorRef<'a>> {
        if (self.0 & Self::TAG_MASK) == Self::TABLE_TAG {
            Some(TableDescriptorRef(self))
        } else {
            None
        }
    }

    /// Tries to create a view of this Level 0 Descriptor as a Block Descriptor.
    ///
    /// Returns a [`BlockDescriptorRef`] referencing `self`, or [`None`] if self does not contain
    /// a Block Descriptor.
    #[allow(unused)]
    pub const fn as_block<'a>(&'a self) -> Option<BlockDescriptorRef<'a>> {
        if self.0 & Self::TAG_MASK == Self::BLOCK_TAG {
            Some(BlockDescriptorRef(self))
        } else {
            None
        }
    }

    /// Creates a Block Descriptor with the given [`GPIAccessType`].
    pub const fn block(gpi: GPIAccessType) -> Self {
        Self(Self::BLOCK_TAG | (gpi as u64 & GPIAccessType::MASK) << 4)
    }
}

/// View of a [`Level0Descriptor`] as a Table Descriptor.
pub(crate) struct TableDescriptorRef<'a>(&'a Level0Descriptor);

impl<'a> TableDescriptorRef<'a> {
    /// Returns the index of the table referenced by this descriptor within the provided L1 buffer.
    pub fn address(&self) -> usize {
        self.0.0 as usize & Level0Descriptor::TABLE_ADDR_MASK
    }
}

impl Debug for TableDescriptorRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Table ({:x?})", self.address()))
    }
}

/// View of a [`Level0Descriptor`] as a Block Descriptor.
#[allow(unused)]
pub(crate) struct BlockDescriptorRef<'a>(&'a Level0Descriptor);

impl<'a> BlockDescriptorRef<'a> {
    declare_accessor!(8:4, gpi, GPIAccessType);
}

impl Debug for BlockDescriptorRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("Block ({:?})", self.gpi()))
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, FromBytes, IntoBytes, KnownLayout, Immutable,
)]
#[repr(transparent)]
pub(crate) struct Level1Descriptor(u64);

impl Level1Descriptor {
    const TAG_MASK: u64 = mask!(4);
    const CONTIG_TAG: u64 = 0b0001;

    /// Tries to create a view of this Level 1 Descriptor as a Contig Descriptor.
    ///
    /// Returns a [`ContiguousDescriptorRef`] referencing `self`, or [`None`] if self does not contain
    /// a Contig Descriptor.
    pub const fn as_contig<'a>(&'a self) -> Option<ContiguousDescriptorRef<'a>> {
        if self.0 & Self::TAG_MASK == Self::CONTIG_TAG {
            Some(ContiguousDescriptorRef(self))
        } else {
            None
        }
    }

    /// Creates a Contiguous Descriptor from the given size and gpi.
    pub fn contig(size: ContigSize, gpi: GPIAccessType) -> Self {
        let size: u64 = size.into();
        let gpi: u64 = gpi.into();

        Self(Self::CONTIG_TAG | (size & ContigSize::MASK) << 8 | (gpi & GPIAccessType::MASK) << 4)
    }

    /// Tries to create a view of this Level 1 Descriptor as a Granule Descriptor.
    ///
    /// Returns a [`GranuleDescriptorRef`] referencing `self`, or [`None`] if self does not contain
    /// a Granule Descriptor.
    #[allow(unused)]
    pub fn as_granule<'a>(&'a self) -> Option<GranuleDescriptorRef<'a>> {
        if GPIAccessType::try_from(self.0 & GPIAccessType::MASK).is_ok() {
            Some(GranuleDescriptorRef(self))
        } else {
            None
        }
    }

    /// Tries to create a view of this Level 1 Descriptor as a Granule Descriptor.
    ///
    /// Returns a [`GranuleDescriptorRefMut`] mutably referencing `self`, or [`None`] if self does not
    /// contain a Granule Descriptor.
    pub fn as_granule_mut<'a>(&'a mut self) -> Option<GranuleDescriptorRefMut<'a>> {
        if GPIAccessType::try_from(self.0 & GPIAccessType::MASK).is_ok() {
            Some(GranuleDescriptorRefMut(self))
        } else {
            None
        }
    }

    /// Creates a Granule Descriptor from the given [`GPIAccessType`]s.
    pub fn granule(gpis: &[GPIAccessType; 16]) -> Self {
        let mut s = Self(0);
        let mut granule = GranuleDescriptorRefMut(&mut s);

        for (i, gpi) in gpis.iter().enumerate() {
            granule.set_gpi(i, *gpi);
        }

        s
    }
}

/// View of a [`Level1Descriptor`] as a Contiguous Descriptor.
pub(crate) struct ContiguousDescriptorRef<'a>(&'a Level1Descriptor);

impl ContiguousDescriptorRef<'_> {
    declare_accessor!(10:8, size, ContigSize);
    declare_accessor!(8:4, gpi, GPIAccessType);
}

impl Debug for ContiguousDescriptorRef<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!(
            "Contiguous ({:?}, {:?})",
            self.size(),
            self.gpi(),
        ))
    }
}

/// View of a [`Level1Descriptor`] as a Granule Descriptor.
pub(crate) struct GranuleDescriptorRef<'a>(&'a Level1Descriptor);

impl<'a> GranuleDescriptorRef<'a> {
    /// Returns the [`GPIAccessType`] corresponding to the granule at index `idx`, or `None` if it
    /// is misprogrammed.
    pub fn gpi(&self, idx: usize) -> Option<GPIAccessType> {
        assert!(idx < 16);

        let start = idx * 4;

        ((self.0.0 >> start) & 0xF).try_into().ok()
    }

    /// Whether all Granules are mapped with [`GPIAccessType::NoAccess`].
    #[allow(unused)]
    pub fn is_empty(&self) -> bool {
        (0..16).all(|idx| self.gpi(idx).is_none_or(|v| v == GPIAccessType::NoAccess))
    }

    /// Whether all Granules are mapped with [`GPIAccessType::Any`].
    #[allow(unused)] // Used only in tests.
    pub fn is_all(&self) -> bool {
        (0..16).all(|idx| self.gpi(idx).is_some_and(|v| v == GPIAccessType::Any))
    }
}

/// Mutable view of a [`Level1Descriptor`] as a Granule Descriptor.
pub(crate) struct GranuleDescriptorRefMut<'a>(&'a mut Level1Descriptor);

impl GranuleDescriptorRefMut<'_> {
    /// Updates the [`GPIAccessType`] for the granule at index `idx`.
    pub fn set_gpi(&mut self, idx: usize, value: GPIAccessType) -> bool {
        assert!(idx < 16);

        let start = idx * 4;
        let value: u64 = value.into();

        self.0.0 = (self.0.0 & !(0xF << start)) | ((value & 0xF) << start);

        true
    }
}

pub(crate) struct Level0Table<'a>(pub(crate) &'a mut [Level0Descriptor]);

impl<'a> Level0Table<'a> {
    /// Get the Level 1 table corresponding to the given PA. The PA must resolve to an Level 1 table.
    ///
    /// # Safety
    ///
    /// `self` must be part of a correctly programmed GPT and `config` must be its configuration.
    pub(crate) unsafe fn get_l1(
        &mut self,
        pa: PA,
        config: &GranuleProtectionConfig,
    ) -> Option<&mut Level1Table> {
        let l0_idx = config.l0_resolve(pa);
        // Safety: since the GPT is correctly programmed, all Table Descriptors point to Level1Table
        // whose size is given by the L0GPTSZ and PGS fields.
        self.0[l0_idx].as_table().map(|d| unsafe {
            from_raw_parts_mut(
                d.address() as *mut _,
                1 << (config.l0gptsz.width() - (config.pgs.width() + 4)),
            )
        })
    }
}

pub(crate) type Level1Table = [Level1Descriptor];

#[cfg(test)]
mod test {

    use crate::{
        Error, GPIAccessType,
        table::{ContigSize, Level0Descriptor, Level1Descriptor},
    };

    #[test]
    fn as_block_valid() {
        assert!(
            Level0Descriptor(0b0001)
                .as_block()
                .is_some_and(|b| b.gpi() == GPIAccessType::NoAccess)
        );

        assert!(
            Level0Descriptor(0b1111_0001)
                .as_block()
                .is_some_and(|b| b.gpi() == GPIAccessType::Any)
        );
    }

    #[test]
    fn as_block_invalid() {
        assert!(Level0Descriptor(0).as_block().is_none());
    }

    #[test]
    fn create_block() {
        assert_eq!(
            Level0Descriptor::block(GPIAccessType::Realm),
            Level0Descriptor(0b1011_0001)
        );
    }

    #[test]
    fn as_table_valid() {
        assert!(Level0Descriptor(0x0001_dead_beef_0003).as_table().is_some());
    }

    #[test]
    fn as_table_invalid() {
        assert!(Level0Descriptor(0x0001_dead_beef_0001).as_table().is_none());
        assert!(Level0Descriptor(0x0001_dead_beef_0005).as_table().is_none());
    }

    #[test]
    fn as_table_idx() {
        let desc = Level0Descriptor(0x1000_0000_2003).as_table().unwrap();
        assert_eq!(desc.address(), 0x1000_0000_2000);

        let desc = Level0Descriptor(0x1000_0001_0003).as_table().unwrap();
        assert_eq!(desc.address(), 0x1000_0001_0000);
    }

    #[test]
    fn as_contig_valid() {
        let desc = Level1Descriptor(0b11_1001_0001).as_contig().unwrap();

        assert_eq!(desc.size(), ContigSize::MB512);
        assert_eq!(desc.gpi(), GPIAccessType::NonSecure);
    }

    #[test]
    fn as_contig_invalid() {
        assert!(Level1Descriptor(0b11_1001_0000).as_contig().is_none());
    }

    #[test]
    fn create_contig() {
        assert_eq!(
            Level1Descriptor::contig(ContigSize::MB2, GPIAccessType::Realm),
            Level1Descriptor(0b01_1011_0001)
        );
    }

    #[test]
    fn as_granule_valid() {
        let gpi = Level1Descriptor(0xB09F).as_granule().unwrap();

        assert_eq!(gpi.gpi(0), Some(GPIAccessType::Any));
        assert_eq!(gpi.gpi(1), Some(GPIAccessType::NonSecure));
        assert_eq!(gpi.gpi(2), Some(GPIAccessType::NoAccess));
        assert_eq!(gpi.gpi(3), Some(GPIAccessType::Realm));
    }

    #[test]
    fn as_granule_mut_valid() {
        let mut desc = Level1Descriptor(0xB09F);
        assert!(desc.as_granule_mut().is_some());
    }

    #[test]
    fn as_granule_invalid() {
        assert!(Level1Descriptor(1).as_granule().is_none());
        assert!(Level1Descriptor(1).as_granule_mut().is_none());
    }

    #[test]
    fn granule_set() {
        let mut desc = Level1Descriptor(0xB09F);
        let mut gpi = desc.as_granule_mut().unwrap();

        gpi.set_gpi(7, GPIAccessType::Root);
        assert_eq!(gpi.0.0, 0x0000_0000_A000_B09F);

        gpi.set_gpi(1, GPIAccessType::Secure);
        gpi.set_gpi(14, GPIAccessType::Secure);
        assert_eq!(gpi.0.0, 0x0800_0000_A000_B08F);
    }

    #[test]
    fn create_granule() {
        assert_eq!(
            Level1Descriptor::granule(&[GPIAccessType::Secure; 16]).0,
            0x8888_8888_8888_8888
        );

        assert_eq!(
            Level1Descriptor::granule(&[
                GPIAccessType::NonSecure,
                GPIAccessType::Root,
                GPIAccessType::Any,
                GPIAccessType::Secure,
                GPIAccessType::NonSecure,
                GPIAccessType::Root,
                GPIAccessType::Any,
                GPIAccessType::Secure,
                GPIAccessType::NonSecure,
                GPIAccessType::Root,
                GPIAccessType::Any,
                GPIAccessType::Secure,
                GPIAccessType::NonSecure,
                GPIAccessType::Root,
                GPIAccessType::Any,
                GPIAccessType::Secure,
            ])
            .0,
            0x8FA9_8FA9_8FA9_8FA9
        );
    }

    #[test]
    fn granule_non_empty() {
        macro_rules! assert_non_empty {
            ($e:expr) => {
                assert!($e.as_granule().is_some_and(|v| !v.is_empty()))
            };
        }

        assert_non_empty!(Level1Descriptor::granule(&[GPIAccessType::Any; 16]));
        assert_non_empty!(Level1Descriptor(0xF000));
    }

    #[test]
    fn granule_empty() {
        macro_rules! assert_empty {
            ($e:expr) => {
                assert!($e.as_granule().is_none_or(|v| v.is_empty()))
            };
        }

        assert_empty!(Level1Descriptor::granule(&[GPIAccessType::NoAccess; 16]));
        assert_empty!(Level1Descriptor(0x3));
        assert_empty!(Level1Descriptor(0x30));
    }

    #[test]
    fn contig_invalid_shift() {
        assert_eq!(ContigSize::from_shift(30), None);
    }
}
