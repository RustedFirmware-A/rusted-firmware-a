// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_ffa::{FfaError, FuncId, Interface, Version};
use log::{error, info};

use crate::{
    context::{switch_world, World},
    services::{owns, Service},
    smccc::{FunctionId, OwningEntityNumber, SmcReturn},
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

    fn handle_smc(
        function: FunctionId,
        x1: u64,
        x2: u64,
        x3: u64,
        x4: u64,
        world: World,
    ) -> SmcReturn {
        let regs = [function.0.into(), x1, x2, x3, x4, 0, 0, 0];

        let msg = match Interface::from_regs(FFA_VERSION_1_1, &regs) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Invalid FF-A call {:#x?}", e);
                return Interface::error(e.into()).into();
            }
        };
        match msg {
            Interface::Version { input_version } => version(world, input_version),
            Interface::MsgWait { .. } => msg_wait(world, 0),
            _ => Interface::error(FfaError::NotSupported),
        }
        .into()
    }
}

/// Returns the version of the Firmware Framework implementation supported by RF-A.
fn version(world: World, _input_version: Version) -> Interface {
    match world {
        // TODO: Implement this properly (Direct Message if needed)
        World::NonSecure => Interface::VersionOut {
            output_version: FFA_VERSION_1_0,
        },
        World::Secure => Interface::VersionOut {
            output_version: FFA_VERSION_1_1,
        },
        #[cfg(feature = "rme")]
        World::Realm => todo!(),
    }
}

fn msg_wait(world: World, _flags: u32) -> Interface {
    match world {
        World::Secure => {
            // TODO: Check flags and possibly update ownership of RX buffer.
            info!("Switching to normal world.");
            switch_world(World::Secure, World::NonSecure);
            // The return value here doesn't actually matter, because we are switching to non-secure
            // world and the secure world saved register values will be overwritten with a new
            // return value before switching back to secure world. This return value will only be
            // seen by secure world if there is a bug where we fail to write an appropriate return
            // value when next we switch to secure world, so make it an error code we can recognise.
            Interface::error(FfaError::Denied)
        }
        _ => {
            // FFA_MSG_WAIT is not allowed over SMC in the non-secure physical instance.
            Interface::error(FfaError::NotSupported)
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
