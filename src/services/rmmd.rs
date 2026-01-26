// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use percore::Cores;

use crate::{
    context::{CoresImpl, World},
    info,
    platform::{Platform, PlatformImpl},
    services::{Service, owns},
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
};

const RMM_BOOT_VERSION: u64 = 0x5;
const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;

/// Arm CCA SMCs, for communication between RF-A and TF-RMM.
///
/// This is described at
/// <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html>
pub struct Rmmd;

impl Service for Rmmd {
    owns! {OwningEntityNumber::STANDARD_SECURE, 0x0150..=0x01CF}

    fn handle_realm_smc(&self, regs: &mut SmcReturn) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            RMM_BOOT_COMPLETE => {
                info!("Realm boot completed with code 0x{:x}", regs.values()[1]);
                self.handle_boot_complete(regs)
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

    fn handle_boot_complete(&self, regs: &mut SmcReturn) -> World {
        let ret = regs.values()[1] as i32;
        regs.set_from(ret);
        World::NonSecure
    }

    pub fn entrypoint_args(&self) -> [u64; 8] {
        let core_linear_id = CoresImpl::core_index() as u64;
        [
            core_linear_id,
            RMM_BOOT_VERSION,
            PlatformImpl::CORE_COUNT as u64,
            PlatformImpl::RMM_SHARED_BUFFER_START as u64,
            0,
            0,
            0,
            0,
        ]
    }
}
