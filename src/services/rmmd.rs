// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::World,
    services::{Service, owns},
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
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

    fn handle_realm_smc(&self, regs: &mut SmcReturn) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            RMM_BOOT_COMPLETE => {
                rmm_boot_complete(regs);
                World::NonSecure
            }
            _ => {
                regs.set_from(NOT_SUPPORTED);
                World::Realm
            }
        }
    }
}

impl Rmmd {
    pub(super) fn new() -> Self {
        Self
    }
}

fn rmm_boot_complete(regs: &mut SmcReturn) {
    let ret = regs.values()[1] as i32;
    regs.set_from(ret);
}
