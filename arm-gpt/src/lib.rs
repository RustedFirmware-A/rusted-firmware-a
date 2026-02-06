// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

// TODO: Temporary until the crate is fully implemented.
#![allow(dead_code)]
#![no_std]

use core::fmt::Debug;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use thiserror::Error;

pub use crate::table::GPIAccessType;
use crate::table::Level0Table;

#[cfg(all(target_arch = "aarch64", not(test)))]
mod aarch64;
mod table;

pub type PA = usize;

/// Errors returned when manipulating the [`GranuleProtection`] object.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum Error {
    #[error("No existing GPT found")]
    GptNotInitialized,
    #[error("Existing GPT Config is invalid")]
    InvalidConfiguration,
    #[error("L0 buffer must be aligned on its size")]
    MisalignedL0Buffer,
}

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
    pub fn width(&self) -> usize {
        match self {
            Self::KB4 => 12,
            Self::KB16 => 14,
            Self::KB64 => 16,
        }
    }
}

/// Size configuration of the [`GranuleProtection`] object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GranuleProtectionConfig {
    /// [`Pps`] used by this [`GranuleProtection`].
    pps: ProtectedPhysicalAddressSize,
    /// [`L0GptSz`] used by this [`GranuleProtection`].
    l0gptsz: Level0GptSize,
    /// [`Pgs`] used by this [`GranuleProtection`].
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
}
