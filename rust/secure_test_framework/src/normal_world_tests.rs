// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in normal world.

use crate::{expect_eq, ffa, normal_world_test, util::NORMAL_WORLD_ID, util::SPMC_DEFAULT_ID};
use arm_ffa::{FfaError, Interface, SuccessArgsIdGet, SuccessArgsSpmIdGet, TargetInfo};
use smccc::{Smc, arch, psci};

normal_world_test!(test_smccc_arch);
fn test_smccc_arch() -> Result<(), ()> {
    expect_eq!(
        arch::version::<Smc>(),
        Ok(arch::Version { major: 1, minor: 5 })
    );
    expect_eq!(arch::features::<Smc>(42), Err(arch::Error::NotSupported));
    Ok(())
}

normal_world_test!(test_psci_version);
fn test_psci_version() -> Result<(), ()> {
    expect_eq!(
        psci::version::<Smc>(),
        Ok(psci::Version { major: 1, minor: 3 })
    );
    Ok(())
}

normal_world_test!(test_no_msg_wait_from_normal_world);
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

normal_world_test!(test_ffa_id_get);
fn test_ffa_id_get() -> Result<(), ()> {
    let id = match ffa::id_get().map_err(|_| ())? {
        Interface::Success { args, .. } => SuccessArgsIdGet::try_from(args).map_err(|_| ())?.id,
        _ => return Err(()),
    };

    expect_eq!(id, NORMAL_WORLD_ID);
    Ok(())
}

normal_world_test!(test_ffa_spm_id_get);
fn test_ffa_spm_id_get() -> Result<(), ()> {
    let id = match ffa::spm_id_get().map_err(|_| ())? {
        Interface::Success { args, .. } => SuccessArgsSpmIdGet::try_from(args).map_err(|_| ())?.id,
        _ => return Err(()),
    };

    // TODO: parse manifest and test for that value.
    expect_eq!(id, SPMC_DEFAULT_ID);
    Ok(())
}
