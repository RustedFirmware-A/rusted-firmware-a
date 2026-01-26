// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::cell::RefCell;

use percore::{Cores, ExceptionLock, PerCore};
use spin::Once;

use crate::{
    context::{CoresImpl, PerCoreState, World},
    info,
    platform::{Platform, PlatformImpl, exception_free},
    services::{Service, owns},
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
};

const RMM_BOOT_VERSION: u64 = 0x5;
const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;

#[derive(Debug)]
struct RmmdLocal {
    activation_token: Option<u64>,
}

impl RmmdLocal {
    const fn new() -> Self {
        Self {
            activation_token: None,
        }
    }
}

pub static RMM_COLD_BOOT_DONE: Once<()> = Once::new();

/// Arm CCA SMCs, for communication between RF-A and TF-RMM.
///
/// This is described at
/// <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html>
pub struct Rmmd {
    core_local: PerCoreState<RmmdLocal>,
}

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
        let core_local = PerCore::new(
            [const { ExceptionLock::new(RefCell::new(RmmdLocal::new())) };
                PlatformImpl::CORE_COUNT],
        );

        Self { core_local }
    }

    /// Initializes the set of registers to pass to R-EL2 after waking up from a suspend.
    ///
    /// <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html#warm-boot-interface>
    pub(crate) fn handle_wake_from_cpu_suspend(&self) -> [u64; 4] {
        let activation_token = exception_free(|token| {
            self.core_local
                .get()
                .borrow(token)
                .borrow()
                .activation_token
                .unwrap_or_default()
        });

        [CoresImpl::core_index() as u64, activation_token, 0, 0]
    }

    fn handle_boot_complete(&self, regs: &mut SmcReturn) -> World {
        let ret = regs.values()[1] as i32;

        if ret != 0 {
            panic!("RMM Boot failed (code: {ret})")
        }

        exception_free(|token| {
            let mut state = self.core_local.get().borrow_mut(token);

            if state.activation_token.is_none() {
                let activation_token = regs.values()[2];
                info!("Received activation token {activation_token:#x?}");
                state.activation_token = Some(activation_token)
            }
        });

        RMM_COLD_BOOT_DONE.call_once(|| ());
        regs.set_from(ret);

        World::NonSecure
    }

    pub fn entrypoint_args(&self) -> [u64; 8] {
        let core_linear_id = CoresImpl::core_index() as u64;
        if RMM_COLD_BOOT_DONE.is_completed() {
            // When warmbooting a PE for the first time, it should only receive the core id as
            // per the RMM-EL3 warmboot interface. Activation token is set to 0 as it was not
            // generated for this core yet. Subsequent warmboot parameters on this PE will be
            // provided by [`Rmmd::handle_wake_from_cpu_suspend`].
            //
            // https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html#warm-boot-interface
            [core_linear_id, 0, 0, 0, 0, 0, 0, 0]
        } else {
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
}
