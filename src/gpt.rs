// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

// TODO: Temporary until the RME feature is fully implemented.
#![allow(unused, dead_code)]

mod aarch64;
mod table;

use crate::aarch64::{dsb_osh, dsb_oshst, tlbi_rpalos};
use core::fmt::Debug;
use num_enum::{IntoPrimitive, TryFromPrimitive};
pub use table::GPIAccessType;
use table::{Level0Table, Level1Descriptor};

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

    /// Updates an access control mapping in the GPT.
    ///
    /// - `base_pa`: Address of the granule whose GPI is updated.
    /// - `gpi`: Describes which Physical Address Space the granule will belong to.
    pub fn set(&mut self, base_pa: PA, gpi: GPIAccessType) -> Result<(), GranuleError> {
        if base_pa >= self.config.pps.size() {
            return Err(GranuleError::InvalidRequest);
        }
        let l0_idx = self.config.l0_resolve(base_pa);
        let l0_entry = self.level0.0[l0_idx];

        // We do not support changing the GPI of an L0 block descriptor.
        if l0_entry.as_block().is_some() {
            return Err(GranuleError::InvalidRequest);
        }
        let Some(mut l0_entry) = l0_entry.as_table() else {
            // Not block or table descriptor
            return Err(GranuleError::InvalidL0Entry);
        };

        // Safety:
        // - l0_entry is an entry from the GPT, and the GPT is assumed to be programmed correctly.
        // - self.config is the config of the GranuleProtection object.
        let l1_table: &mut [Level1Descriptor] = unsafe { l0_entry.to_table_mut(&self.config) };
        let l1_idx = self.config.l1_resolve(base_pa);
        let l1_desc = &mut l1_table[l1_idx];

        // We do not support contiguous descriptors.
        if l1_desc.as_contig().is_some() {
            return Err(GranuleError::InvalidRequest);
        }
        let Some(mut gran) = l1_desc.as_granule_mut() else {
            // Not granule or contig descriptor
            return Err(GranuleError::InvalidL1Entry);
        };

        let gran_idx = self.config.granule_resolve(base_pa);
        gran.set_gpi(gran_idx, gpi);

        dsb_oshst();
        // Ensure that all agents observe the new configuration.
        tlbi_rpalos(base_pa, self.pgs());
        dsb_osh();

        Ok(())
    }

    /// Looks up the access control mapping of the memory region starting at `base_pa` from the GPT.
    pub fn lookup(&self, base_pa: PA) -> Result<GPIAccessType, GranuleError> {
        if base_pa >= self.config.pps.size() {
            return Err(GranuleError::InvalidRequest);
        }
        let l0_idx = self.config.l0_resolve(base_pa);
        let l0_entry = self.level0.0[l0_idx];

        if let Some(block) = l0_entry.as_block() {
            return Ok(block.gpi());
        }
        let Some(l0_entry) = l0_entry.as_table() else {
            // Not block or table descriptor
            return Err(GranuleError::InvalidL0Entry);
        };

        // Safety:
        // - l0_entry is an entry from the GPT, and the GPT is assumed to be programmed correctly.
        // - self.config is the config of the GranuleProtection object.
        let l1_table: &[Level1Descriptor] = unsafe { l0_entry.to_table(&self.config) };
        let l1_idx = self.config.l1_resolve(base_pa);
        let l1_desc = l1_table[l1_idx];

        if let Some(contig) = l1_desc.as_contig() {
            return Ok(contig.gpi());
        }
        let Some(granule) = l1_desc.as_granule() else {
            // Not granule or contiguous descriptor
            return Err(GranuleError::InvalidL1Entry);
        };

        let gran_idx = self.config.granule_resolve(base_pa);
        granule.gpi(gran_idx).ok_or(GranuleError::InvalidL1Entry)
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

    use arm_sysregs::{GpccrEl3, GptbrEl3, fake::SYSREGS};

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
            SYSREGS.lock().unwrap().gptbr_el3 = gptbr;
            SYSREGS.lock().unwrap().gpccr_el3 = GpccrEl3::empty() | GpccrEl3::GPC;
            let mut $name =
                // SAFETY: Each test only calls this once.
                unsafe { GranuleProtection::discover().expect("failed to discover GPT") };
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

    #[test]
    fn gpt_uninit() {
        SYSREGS.lock().unwrap().gpccr_el3 = GpccrEl3::empty();
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

        // SAFETY: GPT is assumed to be programmed correctly.
        assert_eq!(None, unsafe { gpt.level0.get_l1(0x0, &gpc) });

        // Set first descriptor to Block::NoAccess
        write_block!(l0table, 0, GPIAccessType::NoAccess);

        let addr_0 = (1 << 30) - 1;
        assert_eq!(gpc.l0_resolve(addr_0), 0);
        assert_eq!(gpt.lookup(addr_0), Ok(GPIAccessType::NoAccess));

        // Create secure block
        let addr_1 = 1 << 30;
        assert_eq!(gpc.l0_resolve(addr_1), 1);
        write_block!(l0table, 1, GPIAccessType::Secure);
        assert_eq!(gpt.lookup(addr_1), Ok(GPIAccessType::Secure));

        // Create L1 table
        let addr_2 = 2 << 30;
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
        let gr_1 = addr_2 + 0x1000;
        let gr_2 = addr_2 + 0x2000;
        let gr_3 = addr_2 + 0x3000;
        let gr_4 = addr_2 + 0x4000;
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

        declare_empty_gpt!(gpt, l0table, gpc);

        // Create L1 table
        let addr_1 = 1 << 30;
        declare_l1!(l1table, gpc.pgs, gpc.l0gptsz, 32);
        let base = l1table.as_ptr() as u64;
        let desc = Level0Descriptor::table(base);
        let bytes = desc.as_bytes();
        let offset = size_of::<Level0Descriptor>();
        l0table[offset..offset + bytes.len()].copy_from_slice(bytes);
        assert_eq!(gpt.lookup(addr_1), Ok(GPIAccessType::NoAccess));

        // L1 table initialized with zeros means they are granules with NoAccess GPI.
        let gr_0 = addr_1;
        let gr_1 = addr_1 + 0x1000;

        assert_eq!(gpt.lookup(gr_0), Ok(GPIAccessType::NoAccess));
        assert_eq!(gpt.lookup(gr_1), Ok(GPIAccessType::NoAccess));

        // Overwrite L1 table with invalid GPI values
        for val in l1table.iter_mut() {
            *val = 0b0100; // invalid
        }

        assert_eq!(gpt.lookup(gr_0), Err(GranuleError::InvalidL1Entry));
    }
}
