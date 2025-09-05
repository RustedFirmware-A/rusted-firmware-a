// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Types and helpers related to the SMC Calling Convention.

use core::fmt::{self, Debug, Display, Formatter};
use uuid::Uuid;

const FAST_CALL: u32 = 0x8000_0000;
const SMC64: u32 = 0x4000_0000;
const OEN_MASK: u32 = 0x3f00_0000;
const OEN_SHIFT: u8 = 24;
const SVE_HINT: u32 = 1 << 16;
const RESERVED_BITS: u32 = 0x7f << 17;

/// The call completed successfully.
pub const SUCCESS: i32 = 0;

/// The call is not supported by the implementation.
pub const NOT_SUPPORTED: i32 = -1;

/// The call is deemed not required by the implementation.
#[allow(unused)]
pub const NOT_REQUIRED: i32 = -2;

/// One of the call parameters has a non-supported value.
pub const INVALID_PARAMETER: i32 = -3;

/// The type of an SMCCC call: whether it is a fast call or yielding call, and which calling
/// convention it uses.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SmcccCallType {
    /// An SMC32/HVC32 fast call.
    Fast32,
    /// An SMC64/HVC64 fast call.
    Fast64,
    /// A yielding call.
    Yielding,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OwningEntity {
    ArmArchitectureService,
    CPUService,
    SiPService,
    OEMService,
    StandardSecureService,
    StandardHypervisorService,
    VendorSpecificHypervisorService,
    VendorSpecificEL3MonitorService,
    TrustedApplications,
    TrustedOS,
    Unknown,
}

/// Owning Entity Number (OEN)
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct OwningEntityNumber(pub u8);

impl OwningEntityNumber {
    pub const ARM_ARCHITECTURE: Self = Self(0);
    pub const CPU: Self = Self(1);
    pub const SIP: Self = Self(2);
    pub const OEM: Self = Self(3);
    pub const STANDARD_SECURE: Self = Self(4);
    pub const STANDARD_HYPERVISOR: Self = Self(5);
    pub const VENDOR_SPECIFIC_HYPERVISOR: Self = Self(6);
    pub const VENDOR_SPECIFIC_EL3_MONITOR: Self = Self(7);

    pub fn oe(self) -> OwningEntity {
        match self {
            Self::ARM_ARCHITECTURE => OwningEntity::ArmArchitectureService,
            Self::CPU => OwningEntity::CPUService,
            Self::SIP => OwningEntity::SiPService,
            Self::OEM => OwningEntity::OEMService,
            Self::STANDARD_SECURE => OwningEntity::StandardSecureService,
            Self::STANDARD_HYPERVISOR => OwningEntity::StandardHypervisorService,
            Self::VENDOR_SPECIFIC_HYPERVISOR => OwningEntity::VendorSpecificHypervisorService,
            Self::VENDOR_SPECIFIC_EL3_MONITOR => OwningEntity::VendorSpecificEL3MonitorService,
            Self(48..=49) => OwningEntity::TrustedApplications,
            Self(50..=63) => OwningEntity::TrustedOS,
            _ => OwningEntity::Unknown,
        }
    }
}

impl Display for OwningEntityNumber {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An SMCCC function ID.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct FunctionId(pub u32);

impl FunctionId {
    /// Creates a new `FunctionId` from its components.
    pub const fn new(call_type: SmcccCallType, oen: OwningEntityNumber, number: u16) -> Self {
        let type_bits = match call_type {
            SmcccCallType::Fast32 => FAST_CALL,
            SmcccCallType::Fast64 => FAST_CALL | SMC64,
            SmcccCallType::Yielding => 0,
        };
        Self(type_bits | (((oen.0 as u32) << OEN_SHIFT) & OEN_MASK) | (number as u32))
    }

    /// Returns the Owning Entity Number of the function ID.
    pub fn oen(self) -> OwningEntityNumber {
        OwningEntityNumber(((self.0 & OEN_MASK) >> OEN_SHIFT) as u8)
    }

    /// Returns the lower 16 bits of the function ID.
    pub fn number(self) -> u16 {
        self.0 as u16
    }

    /// Returns what type of call this is.
    pub fn call_type(self) -> SmcccCallType {
        if self.0 & FAST_CALL != 0 {
            if self.0 & SMC64 != 0 {
                SmcccCallType::Fast64
            } else {
                SmcccCallType::Fast32
            }
        } else {
            SmcccCallType::Yielding
        }
    }

    /// Returns whether the SVE hint bit is set.
    ///
    /// If this is true, the caller asserts that P0-P15, FFR and the bits with index greater than
    /// 127 in the Z0-Z31 registers do not contain any live state.
    #[allow(unused)]
    pub fn sve_hint(self) -> bool {
        self.0 & SVE_HINT != 0
    }

    /// Sets the SVE hint bit.
    #[allow(unused)]
    pub fn set_sve_hint(&mut self) {
        self.0 |= SVE_HINT
    }

    /// Clears the SVE hint bit.
    pub fn clear_sve_hint(&mut self) {
        self.0 &= !SVE_HINT
    }

    /// Returns false if this is a fast call but has any of bits 17-23 set.
    ///
    /// They are reserved for future use and should always be 0.
    pub fn valid(self) -> bool {
        self.call_type() == SmcccCallType::Yielding || self.0 & RESERVED_BITS == 0
    }
}

impl Display for FunctionId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

impl Debug for FunctionId {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "{:#010x} ({:?} OEN {} {:?})",
            self.0,
            self.call_type(),
            self.oen(),
            self.oen().oe()
        )
    }
}

/// A value which can be returned from an SMC call by writing to the caller's registers.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct SmcReturn {
    /// The number of elements from `values` that are actually used for this return.
    used: usize,
    values: [u64; Self::MAX_VALUES],
}

impl SmcReturn {
    pub const MAX_VALUES: usize = 18;

    pub const EMPTY: Self = Self {
        used: 0,
        values: [0; 18],
    };

    /// Returns a slice containing the used values.
    pub fn values(&self) -> &[u64] {
        &self.values[0..self.used]
    }

    /// Returns a mutable slice containing the used values.
    pub fn values_mut(&mut self) -> &mut [u64] {
        &mut self.values[0..self.used]
    }

    /// Returns true if no values are used.
    pub fn is_empty(&self) -> bool {
        self.used == 0
    }
}

impl Debug for SmcReturn {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "SmcReturn([")?;
        let values = self.values();
        if let Some(first) = values.first() {
            write!(f, "{first:#x}")?;
            for value in &values[1..] {
                write!(f, ", {value:#x}")?;
            }
        }
        write!(f, "])")?;
        Ok(())
    }
}

impl From<()> for SmcReturn {
    fn from(_: ()) -> Self {
        Self::EMPTY
    }
}

impl From<u64> for SmcReturn {
    fn from(value: u64) -> Self {
        Self {
            used: 1,
            values: [value, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }
}

impl From<i64> for SmcReturn {
    fn from(value: i64) -> Self {
        Self::from(value as u64)
    }
}

impl From<u32> for SmcReturn {
    fn from(value: u32) -> Self {
        Self::from(u64::from(value))
    }
}

impl From<i32> for SmcReturn {
    fn from(value: i32) -> Self {
        Self::from(value as u64)
    }
}

/// Acccording to section 5.3 of the SMCCC, UUIDs are returned as a single
/// 128-bit value using the SMC32 calling convention. This value is mapped to
/// argument registers x0-x3 on AArch64 (resp. r0-r3 on AArch32). x0 for example
/// shall hold bytes 0 to 3, with byte 0 in the low-order bits.
impl From<Uuid> for SmcReturn {
    fn from(value: Uuid) -> Self {
        let bytes = value.as_bytes();
        Self::from([
            u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as u64,
            u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as u64,
            u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as u64,
            u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as u64,
        ])
    }
}

macro_rules! smc_return_from_array {
    ($length:literal) => {
        impl From<[u64; $length]> for SmcReturn {
            fn from(value: [u64; $length]) -> Self {
                let mut values = [0; Self::MAX_VALUES];
                values[..$length].copy_from_slice(&value);
                Self {
                    used: $length,
                    values,
                }
            }
        }
    };
}

smc_return_from_array!(2);
smc_return_from_array!(3);
smc_return_from_array!(4);
smc_return_from_array!(5);
smc_return_from_array!(6);
smc_return_from_array!(7);
smc_return_from_array!(8);
smc_return_from_array!(9);
smc_return_from_array!(10);
smc_return_from_array!(11);
smc_return_from_array!(12);
smc_return_from_array!(13);
smc_return_from_array!(14);
smc_return_from_array!(15);
smc_return_from_array!(16);
smc_return_from_array!(17);
smc_return_from_array!(18);
