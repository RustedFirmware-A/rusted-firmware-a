// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in normal world.

use crate::{expect_eq, ffa};
use arm_ffa::{FfaError, Interface, TargetInfo};
use log::{error, info};
use smccc::{arch, psci, Smc};

/// The number of normal world tests.
pub const NORMAL_TEST_COUNT: u64 = 3;

/// Runs the test with the given index.
pub fn run_test(index: u64) -> Result<(), ()> {
    info!("Running normal world test {}", index);
    match index {
        0 => test_smccc_arch(),
        1 => test_psci_version(),
        2 => test_no_msg_wait_from_normal_world(),
        _ => {
            error!("Requested to run unknown test {}", index);
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
