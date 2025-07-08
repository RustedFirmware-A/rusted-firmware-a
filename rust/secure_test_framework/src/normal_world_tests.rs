// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in normal world.

use crate::{
    expect_eq, ffa,
    util::{NORMAL_WORLD_ID, SPMC_DEFAULT_ID},
};
use arm_ffa::{FfaError, Interface, SuccessArgsIdGet, SuccessArgsSpmIdGet, TargetInfo};
use log::{error, info};
use smccc::{Smc, arch, psci};

/// The number of normal world tests.
#[allow(unused)]
pub const NORMAL_TEST_COUNT: u64 = 5;

/// Runs the test with the given index.
#[allow(unused)]
pub fn run_test(index: u64) -> Result<(), ()> {
    info!("Running normal world test {}", index);
    match index {
        0 => test_smccc_arch(),
        1 => test_psci_version(),
        2 => test_no_msg_wait_from_normal_world(),
        3 => test_ffa_id_get(),
        4 => test_ffa_spm_id_get(),
        _ => {
            error!("Requested to run unknown test {}", index);
            Err(())
        }
    }
}

/// Runs the secure world test helper for the normal world test with the given index.
#[allow(unused)]
pub fn run_test_helper(index: u64, args: [u64; 3]) -> Result<[u64; 4], ()> {
    info!("Running secure world test helper {}", index);
    match index {
        _ => {
            error!("Requested to run unknown test helper {}", index);
            Err(())
        }
    }
}

fn test_smccc_arch() -> Result<(), ()> {
    expect_eq!(
        arch::version::<Smc>(),
        Ok(arch::Version { major: 1, minor: 5 })
    );
    expect_eq!(arch::features::<Smc>(42), Err(arch::Error::NotSupported));
    Ok(())
}

fn test_psci_version() -> Result<(), ()> {
    expect_eq!(
        psci::version::<Smc>(),
        Ok(psci::Version { major: 1, minor: 3 })
    );
    Ok(())
}

fn test_no_msg_wait_from_normal_world() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_MSG_WAIT.
    expect_eq!(
        ffa::msg_wait(None),
        Ok(Interface::Error {
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported,
            error_arg: 0
        })
    );
    Ok(())
}

fn test_ffa_id_get() -> Result<(), ()> {
    let id = match ffa::id_get().map_err(|_| ())? {
        Interface::Success { args, .. } => SuccessArgsIdGet::try_from(args).map_err(|_| ())?.id,
        _ => return Err(()),
    };

    expect_eq!(id, NORMAL_WORLD_ID);
    Ok(())
}

fn test_ffa_spm_id_get() -> Result<(), ()> {
    let id = match ffa::spm_id_get().map_err(|_| ())? {
        Interface::Success { args, .. } => SuccessArgsSpmIdGet::try_from(args).map_err(|_| ())?.id,
        _ => return Err(()),
    };

    // TODO: parse manifest and test for that value.
    expect_eq!(id, SPMC_DEFAULT_ID);
    Ok(())
}
