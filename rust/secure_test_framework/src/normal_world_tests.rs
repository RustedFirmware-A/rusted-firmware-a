// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in normal world.

use crate::{
    expect_eq, fail, ffa, normal_world_test,
    timer::{NonSecureTimer, test_timer_helper},
    util::{NORMAL_WORLD_ID, SPMC_DEFAULT_ID, expect_success, log_error},
};
use arm_ffa::{
    FfaError, Interface, RxTxAddr, SuccessArgs, SuccessArgsIdGet, SuccessArgsSpmIdGet, TargetInfo,
};
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

normal_world_test!(test_timer);
fn test_timer() -> Result<(), ()> {
    test_timer_helper::<NonSecureTimer>()
}

normal_world_test!(test_rx_tx_map, handler = rx_tx_map_handler);
fn test_rx_tx_map() -> Result<(), ()> {
    let args = expect_success(log_error(
        "RX_TX_MAP failed",
        ffa::rx_tx_map(RxTxAddr::Addr64 { rx: 0x02, tx: 0x03 }, 1),
    )?)?;
    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn rx_tx_map_handler(interface: Interface) -> Option<Interface> {
    let Interface::RxTxMap { addr, page_cnt } = interface else {
        return None;
    };
    assert_eq!(addr, RxTxAddr::Addr64 { rx: 0x02, tx: 0x03 });
    assert_eq!(page_cnt, 1);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}
