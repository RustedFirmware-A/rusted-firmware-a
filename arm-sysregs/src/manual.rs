// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Manually implemented methods for system register types.

use super::{IdAa64mmfr1El1, IdAa64mmfr2El1, IdAa64mmfr3El1, MpidrEl1};
use crate::{Esr, read_mpidr_el1};
use core::fmt::{self, Debug, Formatter};

impl Debug for Esr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Esr({:#x})", self.0)
    }
}

impl IdAa64mmfr1El1 {
    const VH_SHIFT: u64 = 8;
    const VH_MASK: u64 = 0b1111;
    const VH_SUPPORTED: u64 = 0b0001;

    const HCX_SHIFT: u64 = 40;
    const HCX_MASK: u64 = 0b1111;
    const HCX_SUPPORTED: u64 = 0b0001;

    /// Indicates presence of FEAT_VHE.
    pub fn is_feat_vhe_present(self) -> bool {
        (self.bits() >> Self::VH_SHIFT) & Self::VH_MASK >= Self::VH_SUPPORTED
    }

    /// Indicates presence of FEAT_HCX.
    pub fn is_feat_hcx_present(self) -> bool {
        (self.bits() >> Self::HCX_SHIFT) & Self::HCX_MASK >= Self::HCX_SUPPORTED
    }
}

impl IdAa64mmfr2El1 {
    const CCIDX_SHIFT: u64 = 20;
    const CCIDX_MASK: u64 = 0b1111;
    const CCIDX_64_BIT: u64 = 0b0001;

    /// Checks whether 64-bit format is implemented for all levels of the CCSIDR_EL1.
    pub fn has_64_bit_ccsidr_el1(self) -> bool {
        (self.bits() >> Self::CCIDX_SHIFT) & Self::CCIDX_MASK == Self::CCIDX_64_BIT
    }
}

impl IdAa64mmfr3El1 {
    const TCRX_SHIFT: u64 = 0;
    const TCRX_MASK: u64 = 0b1111;
    const TCRX_SUPPORTED: u64 = 1;

    /// Indicates presence of FEAT_TCR2.
    pub fn is_feat_tcr2_present(self) -> bool {
        (self.bits() >> Self::TCRX_SHIFT) & Self::TCRX_MASK >= Self::TCRX_SUPPORTED
    }
}

impl MpidrEl1 {
    /// Converts a PSCI MPIDR value into the equivalent `MpidrEL1` value.
    ///
    /// This reads the MT and U bits from the current CPU's MPIDR_EL1 value and combines them with
    /// the affinity values from the given `psci_mpidr`.
    ///
    /// This assumes that the MPIDR_EL1 values of all CPUs in a system have the same values for the
    /// MT and U bits.
    pub fn from_psci_mpidr(psci_mpidr: u64) -> Self {
        let mpidr_el1 = read_mpidr_el1();
        Self::from_bits_retain(psci_mpidr) | (mpidr_el1 & (Self::MT | Self::U))
    }
}

/// Cache type enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CacheType {
    /// No cache.
    NoCache = 0b000,
    /// Instruction cache only.
    InstructionOnly = 0b001,
    /// Data cache only.
    DataOnly = 0b010,
    /// Separate instruction and data caches.
    SeparateInstructionAndData = 0b011,
    /// Unified cache.
    Unified = 0b100,
}

impl TryFrom<u64> for CacheType {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(match value {
            0b000 => Self::NoCache,
            0b001 => Self::InstructionOnly,
            0b010 => Self::DataOnly,
            0b011 => Self::SeparateInstructionAndData,
            0b100 => Self::Unified,
            _ => return Err(()),
        })
    }
}

/// Wrapper type for describing cache level in a human readable format, i.e. L3 cache = `CacheLevel(3)`
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLevel(pub(crate) u8);

impl CacheLevel {
    /// Creates new instance.
    pub fn new(level: u8) -> Self {
        assert!((1..8).contains(&level));
        Self(level)
    }

    /// Returns the level value.
    pub fn level(&self) -> u8 {
        self.0
    }
}

impl From<CacheLevel> for u64 {
    fn from(value: CacheLevel) -> Self {
        (value.0 - 1).into()
    }
}

/// An AArch64 exception level.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExceptionLevel {
    /// Exception level 0.
    El0 = 0,
    /// Exception level 1.
    El1 = 1,
    /// Exception level 2.
    El2 = 2,
    /// Exception level 3.
    El3 = 3,
}

/// Values for SPSEL.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StackPointer {
    /// Use SP_EL0.
    El0 = 0,
    /// Use SP_EL1, SP_EL2 or SP_EL3 according to the current exception level.
    ElX = 1,
}
