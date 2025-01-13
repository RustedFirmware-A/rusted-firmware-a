// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use log::info;

use crate::{
    context::{switch_world, World},
    services::{owns, Service},
    smccc::{FunctionId, OwningEntityNumber, SmcReturn, NOT_SUPPORTED},
};

const FUNCTION_NUMBER_MIN: u16 = 0x0060;
const FUNCTION_NUMBER_MAX: u16 = 0x00FF;

const FFA_ERROR: u32 = 0x8400_0060;
const FFA_SUCCESS: u32 = 0x8400_0061;
const FFA_SUCCESS64: u32 = 0xC400_0061;
const FFA_VERSION: u32 = 0x8400_0063;
const FFA_MSG_WAIT: u32 = 0x8400_006B;

const FFA_VERSION_1_1: u32 = 1 << 16 | 1;

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
        _x3: u64,
        _x4: u64,
        world: World,
    ) -> SmcReturn {
        match function.0 {
            FFA_VERSION => version(world, x1 as u32).into(),
            FFA_MSG_WAIT => msg_wait(world, x2 as u32),
            _ => NOT_SUPPORTED.into(),
        }
    }
}

/// Returns the version of the Firmware Framework implementation supported by RF-A.
fn version(_world: World, _input_version: u32) -> u32 {
    FFA_VERSION_1_1
}

fn msg_wait(world: World, _flags: u32) -> SmcReturn {
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
            FfaError::Denied.into()
        }
        _ => {
            // FFA_MSG_WAIT is not allowed over SMC in the non-secure physical instance.
            FfaError::NotSupported.into()
        }
    }
}

/// An error returned from an FF-A function call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
enum FfaError {
    NotSupported = -1,
    InvalidParameters = -2,
    NoMemory = -3,
    Busy = -4,
    Interrupted = -5,
    Denied = -6,
    Retry = -7,
    Aborted = -8,
    NoData = -9,
    NotReady = -10,
}

impl From<FfaError> for SmcReturn {
    fn from(value: FfaError) -> Self {
        [FFA_ERROR.into(), 0, value as u64].into()
    }
}
