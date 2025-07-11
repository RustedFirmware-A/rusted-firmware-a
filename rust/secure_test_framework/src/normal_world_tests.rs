// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in normal world.

use crate::{
    expect_eq, fail, ffa, normal_world_test,
    util::{NORMAL_WORLD_ID, SPMC_DEFAULT_ID, log_error},
};
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
    let id = match log_error("ID_GET failed", ffa::id_get())? {
        Interface::Success { args, .. } => {
            log_error(
                "ID_GET returned invalid arguments",
                SuccessArgsIdGet::try_from(args),
            )?
            .id
        }
        other => fail!("ID_GET returned unexpected interface: {other:?}"),
    };

    expect_eq!(id, NORMAL_WORLD_ID);
    Ok(())
}

normal_world_test!(test_ffa_spm_id_get);
fn test_ffa_spm_id_get() -> Result<(), ()> {
    let id = match log_error("SPM_ID_GET failed", ffa::spm_id_get())? {
        Interface::Success { args, .. } => {
            log_error(
                "SPM_ID_GET returned invalid arguments",
                SuccessArgsSpmIdGet::try_from(args),
            )?
            .id
        }
        other => fail!("SPM_ID_GET returned unexpected interface: {other:?}"),
    };

    // TODO: parse manifest and test for that value.
    expect_eq!(id, SPMC_DEFAULT_ID);
    Ok(())
}
