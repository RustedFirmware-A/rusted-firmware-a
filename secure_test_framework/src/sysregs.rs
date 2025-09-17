// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use bitflags::bitflags;

/// Generates a public function named `$function_name` to read the system register `$sysreg` as a
/// value of type `$type`.
///
/// `safe` should only be specified for system registers which are indeed safe to read.
#[cfg(not(test))]
macro_rules! read_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            let value;
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is
            // always safe to read the given `$sysreg.`
            unsafe {
                core::arch::asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            value
        }
    };
    ($sysreg:ident, $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            let value;
            // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
            unsafe {
                core::arch::asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            value
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, safe $function_name:ident) => {
        pub fn $function_name() -> $type {
            let value: $raw_type;
            // SAFETY: The macro call site's author (i.e. see below) has determined that it is
            // always safe to read the given `$sysreg.`
            unsafe {
                core::arch::asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            <$type>::from_bits_retain(value)
        }
    };
    ($sysreg:ident, $raw_type:ty : $type:ty, $function_name:ident) => {
        pub unsafe fn $function_name() -> $type {
            let value: $raw_type;
            // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
            unsafe {
                core::arch::asm!(
                    concat!("mrs {value}, ", stringify!($sysreg)),
                    options(nostack),
                    value = out(reg) value,
                );
            }
            <$type>::from_bits_retain(value)
        }
    };
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct MpidrEl1: u64 {
        const MT = 1 << 24;
        const U = 1 << 30;
    }
}

impl MpidrEl1 {
    pub const AFF0_MASK: u64 = 0x0000_00ff;
    pub const AFF1_MASK: u64 = 0x0000_ff00;
    pub const AFFINITY_BITS: usize = 8;
    pub const AFF0_SHIFT: u8 = 0;
    pub const AFF1_SHIFT: u8 = 8;
    pub const AFF2_SHIFT: u8 = 16;
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

    pub fn aff0(self) -> u8 {
        (self.bits() >> Self::AFF0_SHIFT) as u8
    }

    pub fn aff1(self) -> u8 {
        (self.bits() >> Self::AFF1_SHIFT) as u8
    }

    pub fn aff2(self) -> u8 {
        (self.bits() >> Self::AFF2_SHIFT) as u8
    }

    pub fn aff3(self) -> u8 {
        (self.bits() >> Self::AFF3_SHIFT) as u8
    }
}

read_sysreg!(mpidr_el1, u64: MpidrEl1, safe read_mpidr_el1);
