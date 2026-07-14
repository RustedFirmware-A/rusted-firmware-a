// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{GranuleProtectionConfig, PA, mask};
use core::fmt::Debug;
use core::slice::{from_raw_parts, from_raw_parts_mut};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
/// Access control restriction that can be applied to a memory region in the GPT.
pub enum GPIAccessType {
    /// No accesses permitted.
    NoAccess = 0b0000,
    /// Accesses permitted to System Agent PA space only.
    /// This encoding is reserved if GPCCR_EL3.SA is 0, or if FEAT_RME_GDI is not implemented.
    /// Accesses are not permitted for the PE, only used to check for invalid encodings.
    SystemAgent = 0b0100,
    /// Accesses permitted to Non-secure Protected PA space only.
    /// This encoding is reserved if GPCCR_EL3.NSP is 0, or if FEAT_RME_GDI is not implemented.
    /// Accesses are not permitted for the PE, only used to check for invalid encodings.
    NonSecureProtected = 0b0101,
    /// No accesses permitted.
    /// This encoding is reserved if GPCCR_EL3.NA6 is 0, or if FEAT_RME_GDI is not implemented.
    NoAccess6 = 0b0110,
    /// No accesses permitted.
    /// This encoding is reserved if GPCCR_EL3.NA7 is 0, or if FEAT_RME_GDI is not implemented.
    NoAccess7 = 0b0111,
    /// Accesses permitted to Secure PA space only.
    /// This encoding is reserved if FEAT_SEL2 is not implemented.
    Secure = 0b1000,
    /// Accesses permitted to Non-secure PA space only.
    NonSecure = 0b1001,
    /// Accesses permitted to Root PA space only.
    Root = 0b1010,
    /// Accesses permitted to Realm PA space only.
    Realm = 0b1011,
    /// Accesses permitted to Non-secure PA space only, by Non-secure or Root Security states.
    /// This encoding is reserved if the Effective value of GPCCR_EL3.NSO is 0, or if FEAT_RME_GPC2
    /// is not implemented.
    NonSecureOnly = 0b1101,
    /// All accesses permitted.
    Any = 0b1111,
}

impl GPIAccessType {
    const MASK: u64 = mask!(4);
}

impl From<GPIAccessType> for u64 {
    fn from(gpi: GPIAccessType) -> Self {
        u8::from(gpi).into()
    }
}

impl TryFrom<u64> for GPIAccessType {
    type Error = ();

    fn try_from(value: u64) -> Result<GPIAccessType, Self::Error> {
        let byte = u8::try_from(value).map_err(|_| ())?;
        byte.try_into().map_err(|_| ())
    }
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

/// Possible views of a [`Level0Descriptor`].
pub(crate) enum Level0DescriptorRef<'a> {
    Block(BlockDescriptorRef<'a>),
    Table(TableDescriptorRef<'a>),
}

impl<'a> Level0DescriptorRef<'a> {
    fn is_valid_block(descriptor: &'a Level0Descriptor) -> bool {
        (descriptor.0 & Level0Descriptor::TAG_MASK) == Level0Descriptor::BLOCK_TAG
            && GPIAccessType::try_from(
                (descriptor.0 >> Level0Descriptor::BLOCK_GPI_SHIFT) & GPIAccessType::MASK,
            )
            .is_ok()
    }

    fn is_valid_table(descriptor: &'a Level0Descriptor) -> bool {
        (descriptor.0 & Level0Descriptor::TAG_MASK) == Level0Descriptor::TABLE_TAG
    }
}

impl<'a> TryFrom<&'a Level0Descriptor> for Level0DescriptorRef<'a> {
    type Error = ();

    fn try_from(descriptor: &'a Level0Descriptor) -> Result<Self, Self::Error> {
        if Self::is_valid_block(descriptor) {
            Ok(Self::Block(BlockDescriptorRef(descriptor)))
        } else if Self::is_valid_table(descriptor) {
            Ok(Self::Table(TableDescriptorRef(descriptor)))
        } else {
            Err(())
        }
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
    const BLOCK_GPI_SHIFT: u64 = 4;

    const TABLE_ADDR_LEN: usize = 52;
    const TABLE_ADDR_ALIGN: usize = 12;
    const TABLE_ADDR_MASK: usize = mask!((Self::TABLE_ADDR_LEN), (Self::TABLE_ADDR_ALIGN));

    /// Creates a Block Descriptor with the given [`GPIAccessType`].
    pub const fn block(gpi: GPIAccessType) -> Self {
        Self(Self::BLOCK_TAG | (gpi as u64 & GPIAccessType::MASK) << Self::BLOCK_GPI_SHIFT)
    }

    /// Creates a Table Descriptor pointing to `addr`.
    /// Used only for manually creating L1 tables in unittests.
    #[allow(unused)]
    pub const fn table(addr: u64) -> Self {
        let mask = Self::TABLE_ADDR_MASK as u64;
        assert!(addr & mask == addr);
        Self(Self::TABLE_TAG | (addr & mask))
    }
}

/// View of a [`Level0Descriptor`] as a Table Descriptor.
pub(crate) struct TableDescriptorRef<'a>(&'a Level0Descriptor);

impl<'a> TableDescriptorRef<'a> {
    /// Returns the index of the table referenced by this descriptor within the provided L1 buffer.
    pub fn address(&self) -> usize {
        self.0.0 as usize & Level0Descriptor::TABLE_ADDR_MASK
    }

    /// Returns the `Level1Table` corresponding to this `TableDescriptorRef`.
    ///
    /// # Safety
    /// Callers must ensure that `self` is a pointing to a valid L1 table.
    /// `config` must be the [`GranuleProtectionConfig`] describing the system's Granule Protection
    /// Table.
    pub unsafe fn to_table_mut(&mut self, config: &GranuleProtectionConfig) -> &mut Level1Table {
        // Safety:
        // - A valid L1 table descriptor's size is given by the L0GPTSZ and PGS fields.
        // - `address` is sufficiently aligned.
        // - The max width for address is 56 bits, the max size of the L1 table is 0x80_0000,
        // the sum of which cannot wrap over isize::MAX.
        // - It is assumed that only one GranuleProtection object is created.
        unsafe {
            from_raw_parts_mut(
                self.address() as *mut _,
                1 << (config.l0gptsz.width() - (config.pgs.width() + 4)),
            )
        }
    }

    /// Returns the `Level1Table` corresponding to this `TableDescriptorRef`.
    ///
    /// # Safety
    /// Callers must ensure that `self` is a pointing to a valid L1 table.
    /// `config` must be the [`GranuleProtectionConfig`] describing the system's Granule Protection
    /// Table.
    pub unsafe fn to_table(&self, config: &GranuleProtectionConfig) -> &Level1Table {
        // Safety:
        // - A valid L1 table descriptor's size is given by the L0GPTSZ and PGS fields.
        // - `address` is sufficiently aligned.
        // - The max width for address is 56 bits, the max size of the L1 table is 0x80_0000,
        // the sum of which cannot wrap over isize::MAX.
        // - It is assumed that only one GranuleProtection object is created.
        unsafe {
            from_raw_parts(
                self.address() as *const _,
                1 << (config.l0gptsz.width() - (config.pgs.width() + 4)),
            )
        }
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

/// Possible views of a [`Level1Descriptor`].
pub(crate) enum Level1DescriptorRef<'a> {
    Granule(GranuleDescriptorRef<'a>),
    Contiguous(ContiguousDescriptorRef<'a>),
}

impl<'a> Level1DescriptorRef<'a> {
    fn is_valid_contig(descriptor: &Level1Descriptor) -> bool {
        (descriptor.0 & Level1Descriptor::TAG_MASK) == Level1Descriptor::CONTIG_TAG
            && GPIAccessType::try_from(
                (descriptor.0 >> Level1Descriptor::CONTIG_GPI_SHIFT) & GPIAccessType::MASK,
            )
            .is_ok()
            && ContigSize::try_from(
                (descriptor.0 >> Level1Descriptor::CONTIG_SIZE_SHIFT) & ContigSize::MASK,
            )
            .is_ok()
    }

    fn is_valid_granule(descriptor: &Level1Descriptor) -> bool {
        (0..64)
            .step_by(4)
            .all(|i| GPIAccessType::try_from((descriptor.0 >> i) & GPIAccessType::MASK).is_ok())
    }
}

impl<'a> TryFrom<&'a Level1Descriptor> for Level1DescriptorRef<'a> {
    type Error = ();

    fn try_from(descriptor: &'a Level1Descriptor) -> Result<Self, Self::Error> {
        if Self::is_valid_contig(descriptor) {
            Ok(Self::Contiguous(ContiguousDescriptorRef(descriptor)))
        } else if Self::is_valid_granule(descriptor) {
            Ok(Self::Granule(GranuleDescriptorRef(descriptor)))
        } else {
            Err(())
        }
    }
}

/// Possible views of a mutable [`Level1Descriptor`].
pub(crate) enum Level1DescriptorRefMut<'a> {
    Granule(GranuleDescriptorRefMut<'a>),
    Contiguous(ContiguousDescriptorRef<'a>),
}

impl<'a> TryFrom<&'a mut Level1Descriptor> for Level1DescriptorRefMut<'a> {
    type Error = ();

    fn try_from(descriptor: &'a mut Level1Descriptor) -> Result<Self, Self::Error> {
        if Level1DescriptorRef::is_valid_contig(descriptor) {
            Ok(Self::Contiguous(ContiguousDescriptorRef(descriptor)))
        } else if Level1DescriptorRef::is_valid_granule(descriptor) {
            Ok(Self::Granule(GranuleDescriptorRefMut(descriptor)))
        } else {
            Err(())
        }
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
    const CONTIG_GPI_SHIFT: u64 = 4;
    const CONTIG_SIZE_SHIFT: u64 = 8;

    /// Creates a Contiguous Descriptor from the given size and gpi.
    pub fn contig(size: ContigSize, gpi: GPIAccessType) -> Self {
        let size: u64 = size.into();
        let gpi: u64 = gpi.into();

        Self(
            Self::CONTIG_TAG
                | (size & ContigSize::MASK) << Self::CONTIG_SIZE_SHIFT
                | (gpi & GPIAccessType::MASK) << Self::CONTIG_GPI_SHIFT,
        )
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
        let l0_desc = &self.0[l0_idx];

        let Ok(Level0DescriptorRef::Table(l0_table)) = l0_desc.try_into() else {
            return None;
        };

        // Safety: since the GPT is correctly programmed, all Table Descriptors point to Level1Table
        // whose size is given by the L0GPTSZ and PGS fields.
        Some(unsafe {
            from_raw_parts_mut(
                l0_table.address() as *mut _,
                1 << (config.l0gptsz.width() - (config.pgs.width() + 4)),
            )
        })
    }
}

pub(crate) type Level1Table = [Level1Descriptor];

#[cfg(test)]
mod tests {
    use super::*;
    use core::panic;

    /// Asserts that a descriptor does not convert to the given descriptor view.
    macro_rules! assert_invalid_descriptor {
        ($e:expr, $p:pat) => {
            assert!(!matches!(($e).try_into(), Ok($p)));
        };
    }

    /// Asserts that a descriptor conversion is valid and matches the given descriptor view.
    macro_rules! assert_valid_descriptor {
        ($e:expr, $p:pat) => {
            assert!(matches!(($e).try_into(), Ok($p)));
        };
        ($e:expr, $p:pat => $b:expr) => {
            match ($e).try_into() {
                Ok($p) => $b,
                Err(e) => panic!("Expected valid descriptor"),
                _ => panic!(concat!(
                    "Expected descriptor ",
                    stringify!($e),
                    " to match ",
                    stringify!($p)
                )),
            }
        };
    }

    #[test]
    fn block_valid() {
        assert_valid_descriptor!(
            &Level0Descriptor(0b0001),
            Level0DescriptorRef::Block(block1) =>
            {
                assert_eq!(block1.gpi(), GPIAccessType::NoAccess);
            }
        );

        assert_valid_descriptor!(
            &Level0Descriptor(0b1111_0001),
            Level0DescriptorRef::Block(block2) =>
            {
                assert_eq!(block2.gpi(), GPIAccessType::Any);
            }
        );
    }

    #[test]
    fn as_block_invalid() {
        assert_invalid_descriptor!(&Level0Descriptor(0), Level0DescriptorRef::Block(_));
        assert_invalid_descriptor!(
            &Level0Descriptor(0b0010_0001),
            Level0DescriptorRef::Block(_)
        );
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
        assert_valid_descriptor!(
            &Level0Descriptor(0x0001_dead_beef_0003),
            Level0DescriptorRef::Table(_)
        );
    }

    #[test]
    fn create_table() {
        assert_valid_descriptor!(
            &Level0Descriptor::table(0x1234_0000),
            Level0DescriptorRef::Table(table) =>
            {
                assert_eq!(table.address(), 0x1234_0000);
            }
        );
    }

    #[test]
    fn as_table_invalid() {
        assert_invalid_descriptor!(
            &Level0Descriptor(0x0001_dead_beef_0001),
            Level0DescriptorRef::Table(_)
        );
        assert_invalid_descriptor!(
            &Level0Descriptor(0x0001_dead_beef_0005),
            Level0DescriptorRef::Table(_)
        );
    }

    #[test]
    fn as_table_idx() {
        assert_valid_descriptor!(
            &Level0Descriptor(0x1000_0000_2003),
            Level0DescriptorRef::Table(desc) =>
            {
                assert_eq!(desc.address(), 0x1000_0000_2000);
            }
        );

        assert_valid_descriptor!(
            &Level0Descriptor(0x1000_0001_0003),
            Level0DescriptorRef::Table(desc) =>
            {
                assert_eq!(desc.address(), 0x1000_0001_0000);
            }
        );
    }

    #[test]
    fn as_contig_valid() {
        assert_valid_descriptor!(
            &Level1Descriptor(0b11_1001_0001),
            Level1DescriptorRef::Contiguous(desc) =>
            {
                assert_eq!(desc.size(), ContigSize::MB512);
                assert_eq!(desc.gpi(), GPIAccessType::NonSecure);
            }
        );
    }

    #[test]
    fn as_contig_invalid() {
        // valid size, valid gpi, invalid contig tag.
        assert_invalid_descriptor!(
            &Level1Descriptor(0b11_1001_0000),
            Level1DescriptorRef::Contiguous(_)
        );

        // invalid size, valid gpi, valid contig tag.
        assert_invalid_descriptor!(
            &Level1Descriptor(0b00_1001_0001),
            Level1DescriptorRef::Contiguous(_)
        );

        // valid size, invalid gpi, valid contig tag.
        assert_invalid_descriptor!(
            &Level1Descriptor(0b01_0010_0001),
            Level1DescriptorRef::Contiguous(_)
        );
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
        assert_valid_descriptor!(
            &Level1Descriptor(0xB09F),
            Level1DescriptorRef::Granule(gpi) =>
            {
                assert_eq!(gpi.gpi(0), Some(GPIAccessType::Any));
                assert_eq!(gpi.gpi(1), Some(GPIAccessType::NonSecure));
                assert_eq!(gpi.gpi(2), Some(GPIAccessType::NoAccess));
                assert_eq!(gpi.gpi(3), Some(GPIAccessType::Realm));
            }
        );
    }

    #[test]
    fn as_granule_mut_valid() {
        assert_valid_descriptor!(
            &mut Level1Descriptor(0xB09F),
            Level1DescriptorRefMut::Granule(_)
        );
    }

    #[test]
    fn as_granule_invalid() {
        assert_invalid_descriptor!(&Level1Descriptor(1), Level1DescriptorRef::Granule(_));
        assert_invalid_descriptor!(&mut Level1Descriptor(1), Level1DescriptorRefMut::Granule(_));
        assert_invalid_descriptor!(&Level1Descriptor(0xB19F), Level1DescriptorRef::Granule(_));
        assert_invalid_descriptor!(
            &mut Level1Descriptor(0xB19F),
            Level1DescriptorRefMut::Granule(_)
        );
    }

    #[test]
    fn granule_set() {
        let mut desc = Level1Descriptor(0xB09F);
        assert_valid_descriptor!(&mut desc, Level1DescriptorRefMut::Granule(mut gpi) => {
            gpi.set_gpi(7, GPIAccessType::Root);
            assert_eq!(gpi.0.0, 0x0000_0000_A000_B09F);

            gpi.set_gpi(1, GPIAccessType::Secure);
            gpi.set_gpi(14, GPIAccessType::Secure);
            assert_eq!(gpi.0.0, 0x0800_0000_A000_B08F);
        });
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
            ($e:expr) => {{
                let granule_desc = $e;
                assert_valid_descriptor!(&granule_desc, Level1DescriptorRef::Granule(granule) => {
                    assert!(!granule.is_empty());
                });
            }};
        }

        assert_non_empty!(Level1Descriptor::granule(&[GPIAccessType::Any; 16]));
        assert_non_empty!(Level1Descriptor(0xF000));
    }

    #[test]
    fn granule_empty() {
        macro_rules! assert_empty {
            ($e:expr) => {{
                let granule_desc = $e;
                match Level1DescriptorRef::try_from(&granule_desc) {
                    Ok(Level1DescriptorRef::Granule(granule)) => assert!(granule.is_empty()),
                    Ok(_) => panic!("Expected empty or invalid granule."),
                    Err(_) => (),
                }
            }};
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
