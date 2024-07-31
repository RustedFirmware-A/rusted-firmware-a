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

/// An SMCCC function ID.
#[derive(Copy, Clone, Eq, PartialEq)]
#[repr(transparent)]
pub struct FunctionId(pub u32);

impl FunctionId {
    /// Returns the Owning Entity Number of the function ID.
    pub fn oen(self) -> u8 {
        ((self.0 & OEN_MASK) >> OEN_SHIFT) as u8
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
            "{:#010x} ({:?} OEN {})",
            self.0,
            self.call_type(),
            self.oen()
        )
    }
}

/// A value which can be returned from an SMC call by writing to the caller's registers.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SmcReturn {
    /// The number of elements from `values` that are actually used for this return.
    used: usize,
    values: [u64; 18],
}

impl SmcReturn {
    /// Returns a slice containing the used values.
    pub fn values(&self) -> &[u64] {
        &self.values[0..self.used]
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
