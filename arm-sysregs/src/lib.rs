// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Access to Arm CPU system registers.

#![cfg_attr(not(any(test, feature = "fakes")), no_std)]

#[cfg(not(any(test, feature = "fakes")))]
mod aarch64;
#[cfg(any(test, feature = "fakes"))]
pub mod fake;

use bitflags::bitflags;

/// Generates public functions named `$read_function_name` and `$write_function_name` to read or
/// write (respectively) a value of type `$type` from/to the system register `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to read from or write
/// any value to.
#[macro_export]
macro_rules! read_write_sysreg {
    ($sysreg:ident, $type:ty $(: $bitflags_type:ty)?, safe $read_function_name:ident, safe $write_function_name:ident $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $read_function_name $(, $fake_sysregs)?);
        $crate::write_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $write_function_name $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty $(: $bitflags_type:ty)?, safe $read_function_name:ident, $write_function_name:ident $(, $fake_sysregs:expr)?
    ) => {
        $crate::read_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $read_function_name $(, $fake_sysregs)?);
        $crate::write_sysreg! {
            $(#[$attributes])*
            $sysreg, $type $(: $bitflags_type)?, $write_function_name $(, $fake_sysregs)?
        }
    };
}

bitflags! {
    /// MPIDR_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct MpidrEl1: u64 {
        /// MT
        const MT = 1 << 24;
        /// U
        const U = 1 << 30;
    }
}

impl MpidrEl1 {
    /// Mask for the Aff0 field.
    pub const AFF0_MASK: u64 = 0xff << Self::AFF0_SHIFT;
    /// Mask for the Aff1 field.
    pub const AFF1_MASK: u64 = 0xff << Self::AFF1_SHIFT;
    /// Mask for the Aff2 field.
    pub const AFF2_MASK: u64 = 0xff << Self::AFF2_SHIFT;
    /// Mask for the Aff3 field.
    pub const AFF3_MASK: u64 = 0xff << Self::AFF3_SHIFT;
    /// Size in bits of the affinity fields.
    pub const AFFINITY_BITS: usize = 8;
    /// Position of the lowest bit in the Aff0 field.
    pub const AFF0_SHIFT: u8 = 0;
    /// Position of the lowest bit in the Aff1 field.
    pub const AFF1_SHIFT: u8 = 8;
    /// Position of the lowest bit in the Aff2 field.
    pub const AFF2_SHIFT: u8 = 16;
    /// Position of the lowest bit in the Aff3 field.
    pub const AFF3_SHIFT: u8 = 32;

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

    /// Returns the value of the Aff0 field.
    pub fn aff0(self) -> u8 {
        (self.bits() >> Self::AFF0_SHIFT) as u8
    }

    /// Returns the value of the Aff1 field.
    pub fn aff1(self) -> u8 {
        (self.bits() >> Self::AFF1_SHIFT) as u8
    }

    /// Returns the value of the Aff2 field.
    pub fn aff2(self) -> u8 {
        (self.bits() >> Self::AFF2_SHIFT) as u8
    }

    /// Returns the value of the Aff3 field.
    pub fn aff3(self) -> u8 {
        (self.bits() >> Self::AFF3_SHIFT) as u8
    }
}

read_sysreg!(mpidr_el1, u64: MpidrEl1, safe read_mpidr_el1, fake::SYSREGS);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_mpidr_el1() {
        assert_eq!(format!("{:?}", MpidrEl1::empty()), "MpidrEl1(0x0)");
        assert_eq!(
            format!("{:?}", MpidrEl1::MT | MpidrEl1::U),
            "MpidrEl1(MT | U)"
        );
        assert_eq!(
            format!("{:?}", MpidrEl1::from_bits_retain(0x12_4134_5678)),
            "MpidrEl1(MT | U | 0x1200345678)"
        );
    }
}
