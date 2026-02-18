// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use num_enum::TryFromPrimitive;

pub const RMI_VERSION: u32 = 0xC400_0150;
pub const RMI_GRANULE_DELEGATE: u32 = 0xC400_0151;
pub const RMI_GRANULE_UNDELEGATE: u32 = 0xC400_0152;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(u8)]
pub enum RmiStatusCode {
    /// Command completed successfully
    Success = 0,
    /// The value of a command input value caused the command to fail
    ErrorInput = 1,
    /// An attribute of a Realm does not match the expected value
    ErrorRealm = 2,
    /// An attribute of a REC does not match the expected value
    ErrorRec = 3,
    /// An RTT walk terminated before reaching the target RTT level, or reached an RTTE with an unexpected value
    ErrorRtt = 4,
}
