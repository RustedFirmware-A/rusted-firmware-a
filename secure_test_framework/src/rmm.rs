// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RMM-EL3 interface, as documented at <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html>.

use num_enum::TryFromPrimitive;

#[allow(unused)]
pub const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;
#[allow(unused)]
pub const RMM_RMI_REQ_COMPLETE: u32 = 0xC400_018F;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[allow(dead_code)]
#[repr(i32)]
pub enum RmmCommandReturnCode {
    Ok = 0,
    Unknown = -1,
    BadAddress = -2,
    BadPas = -3,
    NoMemory = -4,
    InvalidValue = -5,
    Again = -6,
}

impl From<RmmCommandReturnCode> for u64 {
    fn from(value: RmmCommandReturnCode) -> Self {
        // Casting to a wider integer sign-extends.
        value as u64
    }
}
