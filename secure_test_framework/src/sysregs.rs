// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// Implements a similar interface to `bitflags` on some newtype.
macro_rules! bitflagslike {
    ($typename:ty: $inner:ty) => {
        impl $typename {
            pub const fn empty() -> Self {
                Self(0)
            }

            pub const fn bits(self) -> $inner {
                self.0
            }

            pub const fn from_bits_retain(bits: $inner) -> Self {
                Self(bits)
            }
        }

        impl core::ops::Not for $typename {
            type Output = Self;

            fn not(self) -> Self {
                Self(!self.0)
            }
        }

        impl core::ops::BitOr for $typename {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self {
                Self(self.0 | rhs.0)
            }
        }

        impl core::ops::BitOrAssign for $typename {
            fn bitor_assign(&mut self, rhs: Self) {
                *self = *self | rhs
            }
        }

        impl core::ops::BitAnd for $typename {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self {
                Self(self.0 & rhs.0)
            }
        }

        impl core::ops::BitAndAssign for $typename {
            fn bitand_assign(&mut self, rhs: Self) {
                *self = *self & rhs
            }
        }
    };
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct MpidrEl1(u64);

bitflagslike!(MpidrEl1: u64);

impl MpidrEl1 {
    pub const AFF0_MASK: u64 = 0x0000_00ff;
    pub const AFF1_MASK: u64 = 0x0000_ff00;
    pub const AFFINITY_BITS: usize = 8;
    pub const AFF0_SHIFT: u8 = 0;
    pub const AFF1_SHIFT: u8 = 8;
    pub const AFF2_SHIFT: u8 = 16;
    pub const AFF3_SHIFT: u8 = 32;
    pub const MT: Self = Self(1 << 24);
    pub const U: Self = Self(1 << 30);

    /// Converts a PSCI MPIDR value into the equivalent `MpidrEL1` value.
    ///
    /// This reads the MT and U bits from the current CPU's MPIDR_EL1 value and combines them with
    /// the affinity values from the given `psci_mpidr`.
    ///
    /// This assumes that the MPIDR_EL1 values of all CPUs in a system have the same values for the
    /// MT and U bits.
    pub fn from_psci_mpidr(psci_mpidr: u64) -> Self {
        let mpidr_el1 = read_mpidr_el1();
        Self(psci_mpidr) | (mpidr_el1 & (Self::MT | Self::U))
    }

    pub fn aff0(self) -> u8 {
        (self.0 >> Self::AFF0_SHIFT) as u8
    }

    pub fn aff1(self) -> u8 {
        (self.0 >> Self::AFF1_SHIFT) as u8
    }

    pub fn aff2(self) -> u8 {
        (self.0 >> Self::AFF2_SHIFT) as u8
    }

    pub fn aff3(self) -> u8 {
        (self.0 >> Self::AFF3_SHIFT) as u8
    }

    pub fn mt(self) -> bool {
        self & Self::MT != Self::empty()
    }
}

read_sysreg!(mpidr_el1, u64: MpidrEl1, safe read_mpidr_el1);
