// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::World,
    services::{owns, Service},
    smccc::{FunctionId, OwningEntityNumber, SmcReturn, NOT_SUPPORTED},
};

const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RmmBootReturn {
    Success = 0,
    Unknown = -1,
    VersionMismatch = -2,
    CpusOutOfRange = -3,
    CpuIdOutOfRange = -4,
    InvalidSharedBuffer = -5,
    ManifestVersionNotSupported = -6,
    ManifestDataError = -7,
}

impl From<i32> for RmmBootReturn {
    fn from(value: i32) -> Self {
        match value {
            0 => RmmBootReturn::Success,
            -1 => RmmBootReturn::Unknown,
            -2 => RmmBootReturn::VersionMismatch,
            -3 => RmmBootReturn::CpusOutOfRange,
            -4 => RmmBootReturn::CpuIdOutOfRange,
            -5 => RmmBootReturn::InvalidSharedBuffer,
            -6 => RmmBootReturn::ManifestVersionNotSupported,
            -7 => RmmBootReturn::ManifestDataError,
            _ => RmmBootReturn::Unknown,
        }
    }
}

/// Arm CCA SMCs, for communication between RF-A and TF-RMM.
///
/// This is described at
/// https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html.
pub struct Rmmd;

impl Service for Rmmd {
    owns! {OwningEntityNumber::STANDARD_SECURE, 0x0150..=0x01CF}

    fn handle_smc(
        function: FunctionId,
        x1: u64,
        _x2: u64,
        _x3: u64,
        _x4: u64,
        world: World,
    ) -> SmcReturn {
        // Only handle SMCs originating from Realm world.
        if world != World::Realm {
            return NOT_SUPPORTED.into();
        }

        match function.0 {
            RMM_BOOT_COMPLETE => rmm_boot_complete(x1 as i32),
            _ => NOT_SUPPORTED.into(),
        }
    }
}

fn rmm_boot_complete(ret: i32) -> SmcReturn {
    ret.into()
}
