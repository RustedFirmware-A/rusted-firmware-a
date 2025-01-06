// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Types and helpers related to the SMC Calling Convention.

use core::fmt::{self, Debug, Display, Formatter};

const FAST_CALL: u32 = 0x8000_0000;
const SMC64: u32 = 0x4000_0000;
const OEN_MASK: u32 = 0x3f00_0000;
const OEN_SHIFT: u8 = 24;

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
#[derive(Clone, Debug, Default, Eq, PartialEq)]
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
