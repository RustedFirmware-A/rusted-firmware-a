// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FF-A SPMD.

use crate::{
    expect_eq, expect_ffa_interface, fail, ffa, normal_world_test,
    util::{
        NORMAL_WORLD_ID, SPMC_DEFAULT_ID, expect_ffa_mem_retrieve_resp, expect_ffa_success,
        log_error,
    },
};
use arm_ffa::{
    Feature, FfaError, FuncId, Interface, PartitionInfoGetFlags, RxTxAddr, SuccessArgs,
    SuccessArgsFeatures, SuccessArgsIdGet, SuccessArgsSpmIdGet, TargetInfo, Uuid,
    memory_management::{Handle, MemReclaimFlags},
    partition_info::{SuccessArgsPartitionInfoGet, SuccessArgsPartitionInfoGetRegs},
};

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

normal_world_test!(test_rxtx_map, handler = rxtx_map_handler);
fn test_rxtx_map() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RXTX_MAP failed",
        ffa::rxtx_map(RxTxAddr::Addr64 { rx: 0x02, tx: 0x03 }, 1)
    );
    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn rxtx_map_handler(interface: Interface) -> Option<Interface> {
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

normal_world_test!(test_ffa_rxtx_unmap, handler = rxtx_unmap_handler);
fn test_ffa_rxtx_unmap() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RXTX_UNMAP failed",
        ffa::rxtx_unmap(102)
    );
    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn rxtx_unmap_handler(interface: Interface) -> Option<Interface> {
    let Interface::RxTxUnmap { id } = interface else {
        return None;
    };
    assert_eq!(id, 102);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_features, handler = ffa_features_handler);
fn test_ffa_features() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "FEATURES failed",
        ffa::features(Feature::FuncId(FuncId::IdGet), 0)
    );
    let properties = log_error(
        "Retrieving SuccessArgsFeatures failed",
        SuccessArgsFeatures::try_from(args),
    )?
    .properties;

    expect_eq!(properties, [0, 0]);
    Ok(())
}

fn ffa_features_handler(interface: Interface) -> Option<Interface> {
    let Interface::Features {
        feat_id,
        input_properties,
    } = interface
    else {
        return None;
    };

    assert_eq!(feat_id, Feature::FuncId(FuncId::IdGet));
    assert_eq!(input_properties, 0);

    Some(Interface::Success {
        args: SuccessArgsFeatures { properties: [0, 0] }.into(),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_rx_acquire, handler = rx_acquire_handler);
fn test_ffa_rx_acquire() -> Result<(), ()> {
    let args = expect_ffa_interface!(expect_ffa_success, "RX_ACQUIRE failed", ffa::rx_acquire(87));

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn rx_acquire_handler(interface: Interface) -> Option<Interface> {
    let Interface::RxAcquire { vm_id } = interface else {
        return None;
    };

    assert_eq!(vm_id, 87);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_rx_release, handler = rx_release_handler);
fn test_ffa_rx_release() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RX_RELEASE failed",
        ffa::rx_release(1195)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn rx_release_handler(interface: Interface) -> Option<Interface> {
    let Interface::RxRelease { vm_id } = interface else {
        return None;
    };
    assert_eq!(vm_id, 1195);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(
    test_ffa_partition_info_get,
    handler = partition_info_get_handler
);
fn test_ffa_partition_info_get() -> Result<(), ()> {
    let flags = PartitionInfoGetFlags { count_only: false };
    let uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap();

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "PARTITION_INFO_GET failed",
        ffa::partition_info_get(uuid, flags)
    );
    expect_eq!(
        args,
        SuccessArgsPartitionInfoGet {
            count: 3,
            size: Some(12),
        }
        .into()
    );
    Ok(())
}

fn partition_info_get_handler(interface: Interface) -> Option<Interface> {
    let Interface::PartitionInfoGet { uuid, flags } = interface else {
        return None;
    };

    assert_eq!(
        uuid,
        Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap()
    );
    assert_eq!(flags, PartitionInfoGetFlags { count_only: false });

    Some(Interface::Success {
        args: SuccessArgsPartitionInfoGet {
            count: 3,
            size: Some(12),
        }
        .into(),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(
    test_ffa_partition_info_get_regs,
    handler = partition_info_get_regs_handler
);
fn test_ffa_partition_info_get_regs() -> Result<(), ()> {
    let uuid = Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap();

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "PARTITION_INFO_GET_REGS failed",
        ffa::partition_info_get_regs(uuid, 0, 0)
    );

    expect_eq!(
        args,
        SuccessArgsPartitionInfoGetRegs {
            last_index: 3,
            current_index: 3,
            info_tag: 0x64,
            descriptor_data: [3; 15 * 8],
        }
        .into()
    );
    Ok(())
}

fn partition_info_get_regs_handler(interface: Interface) -> Option<Interface> {
    let Interface::PartitionInfoGetRegs {
        uuid,
        start_index,
        info_tag,
    } = interface
    else {
        return None;
    };

    assert_eq!(
        uuid,
        Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap()
    );
    assert_eq!(start_index, 0);
    assert_eq!(info_tag, 0);

    Some(Interface::Success {
        args: SuccessArgsPartitionInfoGetRegs {
            last_index: 3,
            current_index: 3,
            info_tag: 0x64,
            descriptor_data: [3; 15 * 8],
        }
        .into(),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_error, handler = error_handler);
fn test_ffa_error() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "ERROR failed",
        ffa::error(0, FfaError::InvalidParameters, 0)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn error_handler(interface: Interface) -> Option<Interface> {
    let Interface::Error {
        error_arg,
        error_code,
        target_info,
    } = interface
    else {
        return None;
    };

    assert_eq!(error_arg, 0);
    assert_eq!(error_code, FfaError::InvalidParameters);
    assert_eq!(
        target_info,
        TargetInfo {
            vcpu_id: 0,
            endpoint_id: 0
        }
    );

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
    })
}

normal_world_test!(test_ffa_success, handler = success_handler);
fn test_ffa_success() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "SUCCESS failed",
        ffa::success(0, SuccessArgs::Args32([1, 2, 3, 4, 5, 6]))
    );

    expect_eq!(args, SuccessArgs::Args32([6, 5, 4, 3, 2, 1]));
    Ok(())
}

fn success_handler(interface: Interface) -> Option<Interface> {
    let Interface::Success { args, target_info } = interface else {
        return None;
    };

    assert_eq!(args, SuccessArgs::Args32([1, 2, 3, 4, 5, 6]));
    assert_eq!(
        target_info,
        TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0
        }
    );

    Some(Interface::Success {
        args: SuccessArgs::Args32([6, 5, 4, 3, 2, 1]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_run, handler = run_handler);
fn test_ffa_run() -> Result<(), ()> {
    let sp_id: u32 = 0x5;
    let vcpu_id: u32 = 0x7;
    let target_information = sp_id << 16 | vcpu_id;

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RUN failed",
        ffa::run(target_information.into())
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn run_handler(interface: Interface) -> Option<Interface> {
    let Interface::Run { target_info } = interface else {
        return None;
    };

    assert_eq!(
        target_info,
        TargetInfo {
            endpoint_id: 5,
            vcpu_id: 7,
        }
    );

    Some(Interface::Success {
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_mem_donate, handler = mem_donate_handler);
fn test_ffa_mem_donate() -> Result<(), ()> {
    let total_len = 40;
    let frag_len = total_len / 4;

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "MEM_DONATE failed",
        ffa::mem_donate(total_len, frag_len, None)
    );

    let SuccessArgs::Args32(args) = args else {
        return Err(());
    };

    let handle: Handle = Handle::from([args[0], args[1]]);
    expect_eq!(handle, [0x0000_1000, 0x0200_0000].into());
    Ok(())
}

fn mem_donate_handler(interface: Interface) -> Option<Interface> {
    let Interface::MemDonate {
        total_len,
        frag_len,
        buf,
    } = interface
    else {
        return None;
    };

    assert_eq!(total_len, 40);
    assert_eq!(frag_len, 10);
    assert_eq!(buf, None);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0x0000_1000, 0x0200_0000, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_mem_lend, handler = mem_lend_handler);
fn test_ffa_mem_lend() -> Result<(), ()> {
    let total_len = 40;
    let frag_len = total_len / 4;

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "MEM_LEND failed",
        ffa::mem_lend(total_len, frag_len, None)
    );

    let SuccessArgs::Args32(args) = args else {
        return Err(());
    };

    let handle: Handle = Handle::from([args[0], args[1]]);
    expect_eq!(handle, [0x0000_1200, 0x0220_0000].into());
    Ok(())
}

fn mem_lend_handler(interface: Interface) -> Option<Interface> {
    let Interface::MemLend {
        total_len,
        frag_len,
        buf,
    } = interface
    else {
        return None;
    };

    assert_eq!(total_len, 40);
    assert_eq!(frag_len, 10);
    assert_eq!(buf, None);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0x0000_1200, 0x0220_0000, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(test_ffa_mem_share, handler = mem_share_handler);
fn test_ffa_mem_share() -> Result<(), ()> {
    let total_len = 40;
    let frag_len = total_len / 4;

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "MEM_SHARE failed",
        ffa::mem_share(total_len, frag_len, None)
    );

    let SuccessArgs::Args32(args) = args else {
        return Err(());
    };

    let handle: Handle = Handle::from([args[0], args[1]]);
    expect_eq!(handle, [0x0000_1300, 0x0230_0000].into());

    Ok(())
}

fn mem_share_handler(interface: Interface) -> Option<Interface> {
    let Interface::MemShare {
        total_len,
        frag_len,
        buf,
    } = interface
    else {
        return None;
    };

    assert_eq!(total_len, 40);
    assert_eq!(frag_len, 10);
    assert_eq!(buf, None);

    Some(Interface::Success {
        args: SuccessArgs::Args32([0x0000_1300, 0x0230_0000, 0, 0, 0, 0]),
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
    })
}

normal_world_test!(
    test_ffa_mem_retrieve_req,
    handler = mem_retrieve_req_handler
);
fn test_ffa_mem_retrieve_req() -> Result<(), ()> {
    let total_len = 40;
    let frag_len = total_len / 4;

    let args = expect_ffa_interface!(
        expect_ffa_mem_retrieve_resp,
        "MEM_RETRIEVE_REQ failed",
        ffa::mem_retrieve_req(total_len, frag_len, None)
    );

    expect_eq!(args.0, total_len);
    expect_eq!(args.1, frag_len);
    Ok(())
}

fn mem_retrieve_req_handler(interface: Interface) -> Option<Interface> {
    let Interface::MemRetrieveReq {
        total_len,
        frag_len,
        buf,
    } = interface
    else {
        return None;
    };

    assert_eq!(total_len, 40);
    assert_eq!(frag_len, 10);
    assert_eq!(buf, None);

    Some(Interface::MemRetrieveResp {
        total_len: total_len,
        frag_len: frag_len,
    })
}

normal_world_test!(test_ffa_mem_reclaim, handler = mem_reclaim_handler);
fn test_ffa_mem_reclaim() -> Result<(), ()> {
    let handle: [u32; 2] = [0x0000_1000, 0x0200_0000];
    let flags = MemReclaimFlags {
        zero_memory: false,
        time_slicing: false,
    };

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "MEM_RECLAIM failed",
        ffa::mem_reclaim(Handle::from(handle), flags)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

fn mem_reclaim_handler(interface: Interface) -> Option<Interface> {
    let Interface::MemReclaim { flags, handle } = interface else {
        return None;
    };

    assert_eq!(handle, [0x0000_1000, 0x0200_0000].into());
    assert_eq!(
        flags,
        MemReclaimFlags {
            time_slicing: false,
            zero_memory: false
        }
    );

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
    })
}
