// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for the FF-A SPMD. All tests below check proper forwarding to Secure World and back to Normal World (and
//! viceversa) with the correct interfaces. STF BL32 does not currently implement the logic of said interfaces (for
//! example, RXTX mapping/unmapping)

use core::fmt::Error;

use crate::{
    ffa,
    framework::{
        expect::{expect_eq, fail},
        normal_world_test, secure_world_test,
    },
    util::{
        NORMAL_WORLD_ID, SPMC_DEFAULT_ID, expect_ffa_interface, expect_ffa_mem_retrieve_resp,
        expect_ffa_success, log_error,
    },
};
use arm_ffa::{
    Feature, FfaError, FuncId, Interface, MemAddr, MsgSend2Flags, NotificationBindFlags,
    NotificationGetFlags, NotificationSetFlags, PartitionInfoGetFlags, RxTxAddr, SuccessArgs,
    SuccessArgsFeatures, SuccessArgsIdGet, SuccessArgsNotificationGet,
    SuccessArgsNotificationInfoGet, SuccessArgsNotificationInfoGet32, SuccessArgsSpmIdGet,
    TargetInfo, Uuid,
    memory_management::{
        DataAccessPermGetSet, Handle, InstructionAccessPermGetSet, MemPermissionsGetSet,
        MemReclaimFlags,
    },
    partition_info::{SuccessArgsPartitionInfoGet, SuccessArgsPartitionInfoGetRegs},
};

normal_world_test!(test_ffa_no_msg_wait);
/// Check that FFA_MSG_WAIT returns NOT_SUPPORTED as normal world isn't allowed to call FFA_MSG_WAIT.
fn test_ffa_no_msg_wait() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_MSG_WAIT.
    let error = log_error("MSG_WAIT failed", ffa::msg_wait(None))?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_id_get);
/// Check that the FFA_ID_GET interface (and its parameters) gets a correct response from SPMD (this interface is not
/// forwarded to secure world).
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
/// Check that the FFA_SPM_ID_GET interface (and its parameters) gets a correct response from SPMD (this interface is not
/// forwarded to secure world).
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

normal_world_test!(test_ffa_rxtx_map, handler = rxtx_map_handler);
/// Check that the FFA_RXTX_MAP interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
fn test_ffa_rxtx_map() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RXTX_MAP failed",
        ffa::rxtx_map(RxTxAddr::Addr64 { rx: 0x02, tx: 0x03 }, 1)
    );
    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_RXTX_UNMAP interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
fn test_ffa_rxtx_unmap() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RXTX_UNMAP failed",
        ffa::rxtx_unmap(102)
    );
    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_FEATURES interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
/// Currently, this test checks that the SPMD returns success and does not check for specific properties.
/// TODO: update with more specific tests when FFA_FEATURES is implemented more completely.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_RX_ACQUIRE interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
fn test_ffa_rx_acquire() -> Result<(), ()> {
    let args = expect_ffa_interface!(expect_ffa_success, "RX_ACQUIRE failed", ffa::rx_acquire(87));

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_RX_RELEASE interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
fn test_ffa_rx_release() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "RX_RELEASE failed",
        ffa::rx_release(1195)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_PARTITION_INFO_GET interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_PARTITION_INFO_GET_REGS interface (and its parameters) is successfully forwarded from normal world
/// to secure world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_ERROR interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
fn test_ffa_error() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "ERROR failed",
        ffa::error(0, FfaError::InvalidParameters, 0)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_SUCCESS interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
fn test_ffa_success() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "SUCCESS failed",
        ffa::success(0, SuccessArgs::Args32([1, 2, 3, 4, 5, 6]))
    );

    expect_eq!(args, SuccessArgs::Args32([6, 5, 4, 3, 2, 1]));
    Ok(())
}

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_RUN interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_MEM_DONATE interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_MEM_LEND interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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
/// Check that the FFA_MEM_SHARE interface (and its parameters) is successfully forwarded from normal world to secure
/// world and back.
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

/// Check that the interface values forwarded from normal world match the expected ones.
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

// Check that the FFA_MEM_RETRIEVE_REQ interface (and its parameters) is successfully forwarded from normal world to
// secure world and back.
// Check that we get a FFA_MEM_RETRIEVE_RESP as a response from secure world and that it contains the same parameters
// that were sent.
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

/// Check that the interface values forwarded from normal world match the expected ones.
fn msg_send2_handler(interface: Interface) -> Option<Interface> {
    let Interface::MsgSend2 {
        flags,
        sender_vm_id,
    } = interface
    else {
        return None;
    };

    assert_eq!(sender_vm_id, 0x03);
    assert_eq!(
        flags,
        MsgSend2Flags {
            delay_schedule_receiver: true
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

normal_world_test!(test_ffa_msg_send2, handler = msg_send2_handler);
fn test_ffa_msg_send2() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "MSG_SEND2 failed",
        ffa::msg_send2(
            0x03,
            MsgSend2Flags {
                delay_schedule_receiver: true
            }
        )
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

// Check that the interface values forwarded from normal world match the expected ones.
// Return a FFA_MEM_RETRIEVE_RESP with the same values that were received to emulate what would be returned by secure
// world
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

// Check that the FFA_MEM_RECLAIM interface (and its parameters) is successfully forwarded from normal world to secure
// world and back.
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

// Check that the interface values forwarded from normal world match the expected ones.
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

// Check that the interface values forwarded from normal world match the expected ones.
fn notification_bitmap_destroy_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationBitmapDestroy { vm_id } = interface else {
        return None;
    };

    assert_eq!(vm_id, 5035);

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
    })
}

normal_world_test!(
    test_ffa_notification_bitmap_destroy,
    handler = notification_bitmap_destroy_handler
);
fn test_ffa_notification_bitmap_destroy() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_BITMAP_DESTROY failed",
        ffa::notification_bitmap_destroy(5035)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

// Check that the interface values forwarded from normal world match the expected ones.
fn notification_bind_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationBind {
        bitmap,
        flags,
        receiver_id,
        sender_id,
    } = interface
    else {
        return None;
    };

    assert_eq!(sender_id, 0x8003);
    assert_eq!(receiver_id, 0x8004);
    assert_eq!(bitmap, 0x1051006008009030);
    assert_eq!(
        flags,
        NotificationBindFlags {
            per_vcpu_notification: false
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

normal_world_test!(
    test_ffa_notification_bind,
    handler = notification_bind_handler
);
fn test_ffa_notification_bind() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_UNBIND failed",
        ffa::notification_bind(
            0x8003,
            0x8004,
            NotificationBindFlags {
                per_vcpu_notification: false,
            },
            0x1051006008009030,
        )
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

// Check that the interface values forwarded from normal world match the expected ones.
fn notification_unbind_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationUnbind {
        bitmap,
        receiver_id,
        sender_id,
    } = interface
    else {
        return None;
    };

    assert_eq!(sender_id, 0x8004);
    assert_eq!(receiver_id, 0x8003);
    assert_eq!(bitmap, 0xff53);

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
    })
}

normal_world_test!(
    test_ffa_notification_unbind,
    handler = notification_unbind_handler
);
fn test_ffa_notification_unbind() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_UNBIND failed",
        ffa::notification_unbind(0x8004, 0x8003, 0xff53)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

// Check that the interface values forwarded from normal world match the expected ones.
fn notification_set_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationSet {
        bitmap,
        flags,
        receiver_id,
        sender_id,
    } = interface
    else {
        return None;
    };

    assert_eq!(sender_id, 0x8017);
    assert_eq!(receiver_id, 0x8044);
    assert_eq!(
        flags,
        NotificationSetFlags {
            delay_schedule_receiver: true,
            vcpu_id: Some(5)
        }
    );
    assert_eq!(bitmap, 0x0051006388009530);

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
    })
}

normal_world_test!(
    test_ffa_notification_set,
    handler = notification_set_handler
);
fn test_ffa_notification_set() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_SET failed",
        ffa::notification_set(
            0x8017,
            0x8044,
            NotificationSetFlags {
                delay_schedule_receiver: true,
                vcpu_id: Some(5),
            },
            0x0051006388009530,
        )
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

// Check that the interface values forwarded from normal world match the expected ones.
// Return a SuccessArgsNotificationGet with some sensible values.
fn notification_get_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationGet {
        flags,
        endpoint_id,
        vcpu_id,
    } = interface
    else {
        return None;
    };

    assert_eq!(vcpu_id, 17);
    assert_eq!(endpoint_id, 44);
    assert_eq!(
        flags,
        NotificationGetFlags {
            sp_bitmap_id: true,
            vm_bitmap_id: false,
            spm_bitmap_id: true,
            hyp_bitmap_id: true,
        }
    );

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgsNotificationGet {
            sp_notifications: Some(1),
            vm_notifications: None,
            spm_notifications: None,
            hypervisor_notifications: None,
        }
        .into(),
    })
}

normal_world_test!(
    test_ffa_notification_get,
    handler = notification_get_handler
);
fn test_ffa_notification_get() -> Result<(), ()> {
    let notification_get_flags = NotificationGetFlags {
        sp_bitmap_id: true,
        vm_bitmap_id: false,
        spm_bitmap_id: true,
        hyp_bitmap_id: true,
    };

    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_GET failed",
        ffa::notification_get(17, 44, notification_get_flags.clone(),)
    );

    let args: Result<SuccessArgsNotificationGet, arm_ffa::Error> =
        SuccessArgsNotificationGet::try_from((notification_get_flags, args));

    match args {
        Err(_) => Err(()),
        Ok(args) => {
            expect_eq!(args.sp_notifications, Some(1));
            expect_eq!(args.vm_notifications, None);
            expect_eq!(args.spm_notifications, Some(0));
            expect_eq!(args.hypervisor_notifications, Some(0));

            Ok(())
        }
    }
}

// Check that the interface values forwarded from normal world match the expected ones.
// Return a SuccessArgsNotificationInfoGet with some sensible values.
fn notification_info_get_handler(interface: Interface) -> Option<Interface> {
    let Interface::NotificationInfoGet { is_32bit } = interface else {
        return None;
    };

    assert_eq!(is_32bit, false);

    Some(Interface::Success {
        target_info: TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        },
        args: SuccessArgsNotificationInfoGet32::default().into(),
    })
}
normal_world_test!(
    test_ffa_notification_info_get,
    handler = notification_info_get_handler
);
fn test_ffa_notification_info_get() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "NOTIFICATION_INFO_GET failed",
        ffa::notification_info_get(false)
    );

    let args: Result<SuccessArgsNotificationInfoGet32, arm_ffa::Error> =
        SuccessArgsNotificationInfoGet::try_from(args);

    match args {
        Err(_) => Err(()),
        Ok(args) => {
            expect_eq!(args.more_pending_notifications, false);
            Ok(())
        }
    }
}

normal_world_test!(test_ffa_no_mem_perm_get);
fn test_ffa_no_mem_perm_get() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_MEM_PERM_GET.
    let error = log_error(
        "MEM_PERM_GET failed",
        ffa::mem_perm_get(MemAddr::Addr64(0x6000_0000), 1),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_mem_perm_set);
fn test_ffa_no_mem_perm_set() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_MEM_PERM_SET.
    let error = log_error(
        "MEM_PERM_SET failed",
        ffa::mem_perm_set(
            MemAddr::Addr64(0x6000_0000),
            MemPermissionsGetSet {
                data_access: DataAccessPermGetSet::ReadOnly,
                instr_access: InstructionAccessPermGetSet::Executable,
            },
            1,
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_el3_intr_handle);
fn test_ffa_no_el3_intr_handle() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_EL3_INTR.
    let error = log_error("EL3_INTR_HANDLE failed", ffa::el3_intr_handle())?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_mem_relinquish);
fn test_ffa_no_mem_relinquish() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_MEM_RELINQUISH.
    let error = log_error("MEM_RELINQUISH failed", ffa::mem_relinquish())?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_secondary_ep_register);
fn test_ffa_no_secondary_ep_register() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_SECONDARY_EP_REGISTER.

    // SAFETY: this is a negative test the entrypoint address won't be used and it does not matter if it's valid or not.
    let error = log_error("SECONDARY_EP_REGISTER failed", unsafe {
        ffa::secondary_ep_register(0x6000_0000)
    })?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_normal_world_resume);
fn test_ffa_no_normal_world_resume() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_NORMAL_WORLD_RESUME.
    let error = log_error("NORMAL_WORLD_RESUME failed", ffa::normal_world_resume())?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_normal_world_resume);
/// Try to resume normal world execution. Since normal world was not preempted in the first place, this should fail.
fn test_ffa_normal_world_resume() -> Result<(), ()> {
    let error = log_error("NORMAL_WORLD_RESUME failed", ffa::normal_world_resume())?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::Denied
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_features_secure);
/// Test FFA_FEATURE interface from secure world.
/// Currently, this test checks that the SPMD returns success.
/// TODO: update with more specific tests when FFA_FEATURES is implemented more completely.
fn test_ffa_features_secure() -> Result<(), ()> {
    let args = expect_ffa_interface!(
        expect_ffa_success,
        "FEATURES failed",
        ffa::features(Feature::FuncId(FuncId::IdGet), 0)
    );

    expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
    Ok(())
}

normal_world_test!(test_ffa_no_yield);
fn test_ffa_no_yield() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_YIELD.
    let error = log_error("YIELD failed", ffa::yield_ffa())?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

normal_world_test!(test_ffa_no_interrupt);
fn test_ffa_no_interrupt() -> Result<(), ()> {
    // Normal world isn't allowed to call FFA_INTERRUPT.

    let error = log_error(
        "INTERRUPT failed",
        ffa::interrupt(
            TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0,
            },
            5,
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_rxtx_map_secure);
fn test_ffa_no_rxtx_map_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_RXTX_MAP.
    let error = log_error(
        "RXTX_MAP failed",
        ffa::rxtx_map(arm_ffa::RxTxAddr::Addr64 { rx: 0x3, tx: 0x4 }, 1),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_rxtx_unmap_secure);
fn test_ffa_no_rxtx_unmap_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_RXTX_UNMAP.
    let error = log_error("RXTX_UNMAP failed", ffa::rxtx_unmap(14))?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_rx_release_secure);
fn test_ffa_no_rx_release_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_RX_RELEASE.
    let error = log_error("RX_RELEASE failed", ffa::rx_release(13))?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_rx_acquire_secure);
fn test_ffa_no_rx_acquire_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_RX_ACQUIRE.
    let error = log_error("RX_ACQUIRE failed", ffa::rx_acquire(39))?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_partition_info_get_secure);
fn test_ffa_no_partition_info_get_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_PARTITION_INFO_GET.
    let error = log_error(
        "PARTITION_INFO_GET failed",
        ffa::partition_info_get(
            Uuid::parse_str("a1a2a3a4b1b2c1c2d1d2d3d4d5d6d7d8").unwrap(),
            PartitionInfoGetFlags { count_only: true },
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_bitmap_create_secure);
fn test_ffa_no_notification_bitmap_create_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_BITMAP_CREATE.
    let error = log_error(
        "NOTIFICATION_BITMAP_CREATE failed",
        ffa::notification_bitmap_create(61, 8),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_bitmap_destroy_secure);
fn test_ffa_no_notification_bitmap_destroy_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_BITMAP_DESTROY.
    let error = log_error(
        "NOTIFICATION_BITMAP_DESTROY failed",
        ffa::notification_bitmap_destroy(54),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_bind_secure);
fn test_ffa_no_notification_bind_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_BIND.
    let error = log_error(
        "NOTIFICATION_BIND failed",
        ffa::notification_bind(
            44,
            7,
            NotificationBindFlags {
                per_vcpu_notification: true,
            },
            0x57832,
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_unbind_secure);
fn test_ffa_no_notification_unbind_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_UNBIND.
    let error = log_error(
        "NOTIFICATION_UNBIND failed",
        ffa::notification_unbind(7, 44, 0x57832),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_get_secure);
fn test_ffa_no_notification_get_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_GET.
    let error = log_error(
        "NOTIFICATION_GET failed",
        ffa::notification_get(
            7,
            44,
            NotificationGetFlags {
                sp_bitmap_id: true,
                vm_bitmap_id: true,
                spm_bitmap_id: true,
                hyp_bitmap_id: true,
            },
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_set_secure);
fn test_ffa_no_notification_set_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_SET.
    let error = log_error(
        "NOTIFICATION_SET failed",
        ffa::notification_set(
            7,
            44,
            NotificationSetFlags {
                delay_schedule_receiver: false,
                vcpu_id: Some(34),
            },
            0x66721,
        ),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_notification_info_get_secure);
fn test_ffa_no_notification_info_get_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_NOTIFICATION_INFO_GET.
    let error = log_error(
        "NOTIFICATION_INFO_GET failed",
        ffa::notification_info_get(false),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}

secure_world_test!(test_ffa_no_run_secure);
fn test_ffa_no_run_secure() -> Result<(), ()> {
    // Secure world isn't allowed to call FFA_RUN.
    let error = log_error(
        "RUN failed",
        ffa::run(TargetInfo {
            endpoint_id: 0,
            vcpu_id: 0,
        }),
    )?;

    expect_eq!(
        error,
        Interface::Error {
            error_arg: 0,
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0
            },
            error_code: FfaError::NotSupported
        }
    );
    Ok(())
}
