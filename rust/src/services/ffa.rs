// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_ffa::{FfaError, FuncId, Interface, Version};
use log::{error, info};

use crate::{
    context::World,
    services::{owns, Service},
    smccc::{OwningEntityNumber, SmcReturn},
};

const FUNCTION_NUMBER_MIN: u16 = 0x0060;
const FUNCTION_NUMBER_MAX: u16 = 0x00FF;

const FFA_VERSION_1_0: Version = Version(1, 0);
const FFA_VERSION_1_1: Version = Version(1, 1);

/// Arm Firmware Framework for A-Profile.
pub struct Ffa;

impl Service for Ffa {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_non_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        Ffa::handle_smc(regs, World::NonSecure)
    }

    fn handle_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        Ffa::handle_smc(regs, World::Secure)
    }
}

impl Ffa {
    pub(super) fn new() -> Self {
        Self
    }

    fn handle_smc(regs: &[u64; 18], world: World) -> (SmcReturn, World) {
        // TODO: forward SVE hint bit
        let msg = match Interface::from_regs(FFA_VERSION_1_1, &regs[..8]) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Invalid FF-A call {:#x?}", e);
                return (Interface::error(e.into()).into(), world);
            }
        };

        let (resp, next_world) = match msg {
            Interface::Version { input_version } => version(world, input_version),
            Interface::MsgWait { .. } => msg_wait(world, 0),
            _ => (Interface::error(FfaError::NotSupported), world),
        };

        (resp.into(), next_world)
    }
}

/// Returns the version of the Firmware Framework implementation supported by RF-A.
fn version(world: World, _input_version: Version) -> (Interface, World) {
    (
        match world {
            // TODO: Implement this properly (Direct Message if needed)
            World::NonSecure => Interface::VersionOut {
                output_version: FFA_VERSION_1_0,
            },
            World::Secure => Interface::VersionOut {
                output_version: FFA_VERSION_1_1,
            },
            #[cfg(feature = "rme")]
            World::Realm => panic!("version call from realm world"),
        },
        world,
    )
}

fn msg_wait(world: World, _flags: u32) -> (Interface, World) {
    match world {
        World::Secure => {
            // TODO: Check flags and possibly update ownership of RX buffer.
            info!("Switching to normal world.");
            (Interface::error(FfaError::Denied), World::NonSecure)
        }
        _ => {
            // FFA_MSG_WAIT is not allowed over SMC in the non-secure physical instance.
            (Interface::error(FfaError::NotSupported), world)
        }
    }
}

impl From<FfaError> for SmcReturn {
    fn from(value: FfaError) -> Self {
        [FuncId::Error as u64, 0, value as u64].into()
    }
}

impl From<Interface> for SmcReturn {
    fn from(interface: Interface) -> Self {
        let mut smc_return: [u64; 8] = [0; 8];
        interface.to_regs(FFA_VERSION_1_1, &mut smc_return);
        SmcReturn::from(smc_return)
    }
}
