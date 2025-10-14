// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_ffa::{
    DirectMsgArgs, Error, Feature, FfaError, Interface, MemAddr, MemOpBuf, MsgSend2Flags,
    MsgWaitFlags, NotificationBindFlags, NotificationGetFlags, NotificationSetFlags,
    PartitionInfoGetFlags, RxTxAddr, SecondaryEpRegisterAddr, SuccessArgs, TargetInfo, Uuid,
    Version,
    memory_management::{Handle, MemPermissionsGetSet, MemReclaimFlags},
};
use smccc::{arch, error::positive_or_error_32, smc64};

/// The FF-A version which we implement here.
const FFA_VERSION: Version = Version(1, 2);

pub fn version(input_version: Version) -> Result<Version, arch::Error> {
    assert_eq!(input_version.0 & 0x8000, 0);
    let output_version = positive_or_error_32::<arch::Error>(
        call_raw(Interface::Version { input_version })[0] as u32,
    )?;
    Ok(Version::try_from(output_version).unwrap())
}

pub fn id_get() -> Result<Interface, Error> {
    call(Interface::IdGet)
}

pub fn spm_id_get() -> Result<Interface, Error> {
    call(Interface::SpmIdGet)
}

pub fn features(feat_id: Feature, input_properties: u32) -> Result<Interface, Error> {
    call(Interface::Features {
        feat_id,
        input_properties,
    })
}

pub fn rxtx_map(addr: RxTxAddr, page_cnt: u32) -> Result<Interface, Error> {
    call(Interface::RxTxMap { addr, page_cnt })
}

pub fn rxtx_unmap(id: u16) -> Result<Interface, Error> {
    call(Interface::RxTxUnmap { id })
}

pub fn rx_acquire(vm_id: u16) -> Result<Interface, Error> {
    call(Interface::RxAcquire { vm_id })
}

pub fn rx_release(vm_id: u16) -> Result<Interface, Error> {
    call(Interface::RxRelease { vm_id })
}

pub fn partition_info_get(uuid: Uuid, flags: PartitionInfoGetFlags) -> Result<Interface, Error> {
    call(Interface::PartitionInfoGet { uuid, flags })
}

pub fn msg_wait(flags: Option<MsgWaitFlags>) -> Result<Interface, Error> {
    call(Interface::MsgWait { flags })
}

pub fn run(target_info: TargetInfo) -> Result<Interface, Error> {
    call(Interface::Run { target_info })
}

/// Sends a direct message request.
#[allow(unused)]
pub fn direct_request(
    source: u16,
    destination: u16,
    args: DirectMsgArgs,
) -> Result<Interface, Error> {
    call(Interface::MsgSendDirectReq {
        src_id: source,
        dst_id: destination,
        args,
    })
}

/// Sends a direct message response.
#[allow(unused)]
pub fn direct_response(
    source: u16,
    destination: u16,
    args: DirectMsgArgs,
) -> Result<Interface, Error> {
    call(Interface::MsgSendDirectResp {
        src_id: source,
        dst_id: destination,
        args,
    })
}

pub fn mem_donate(
    total_len: u32,
    frag_len: u32,
    buf: Option<MemOpBuf>,
) -> Result<Interface, Error> {
    call(Interface::MemDonate {
        total_len,
        frag_len,
        buf,
    })
}

pub fn mem_lend(total_len: u32, frag_len: u32, buf: Option<MemOpBuf>) -> Result<Interface, Error> {
    call(Interface::MemLend {
        total_len,
        frag_len,
        buf,
    })
}

pub fn mem_share(total_len: u32, frag_len: u32, buf: Option<MemOpBuf>) -> Result<Interface, Error> {
    call(Interface::MemShare {
        total_len,
        frag_len,
        buf,
    })
}

pub fn mem_retrieve_req(
    total_len: u32,
    frag_len: u32,
    buf: Option<MemOpBuf>,
) -> Result<Interface, Error> {
    call(Interface::MemRetrieveReq {
        total_len,
        frag_len,
        buf,
    })
}

pub fn mem_reclaim(handle: Handle, flags: MemReclaimFlags) -> Result<Interface, Error> {
    call(Interface::MemReclaim { handle, flags })
}

pub fn success(target_info: u32, args: SuccessArgs) -> Result<Interface, Error> {
    call(Interface::Success {
        target_info: target_info.into(),
        args,
    })
}

pub fn error(target_info: u32, error_code: FfaError, error_arg: u32) -> Result<Interface, Error> {
    call(Interface::Error {
        target_info: target_info.into(),
        error_code,
        error_arg,
    })
}

pub fn normal_world_resume() -> Result<Interface, Error> {
    call(Interface::NormalWorldResume {})
}

pub fn yield_ffa() -> Result<Interface, Error> {
    call(Interface::Yield)
}

pub fn msg_send2(sender_vm_id: u16, flags: MsgSend2Flags) -> Result<Interface, Error> {
    call(Interface::MsgSend2 {
        sender_vm_id,
        flags,
    })
}

pub fn interrupt(target_info: TargetInfo, interrupt_id: u32) -> Result<Interface, Error> {
    call(Interface::Interrupt {
        target_info,
        interrupt_id,
    })
}

/// # Safety
///
/// `addr` must be a valid secondary entry point where it's safe for RF-A to jump to.
pub unsafe fn secondary_ep_register(addr: u64) -> Result<Interface, Error> {
    call(Interface::SecondaryEpRegister {
        entrypoint: SecondaryEpRegisterAddr::Addr64(addr),
    })
}

pub fn partition_info_get_regs(
    uuid: Uuid,
    start_index: u16,
    info_tag: u16,
) -> Result<Interface, Error> {
    call(Interface::PartitionInfoGetRegs {
        uuid,
        start_index,
        info_tag,
    })
}

pub fn notification_bitmap_create(vm_id: u16, vcpu_cnt: u32) -> Result<Interface, Error> {
    call(Interface::NotificationBitmapCreate { vm_id, vcpu_cnt })
}

pub fn notification_bitmap_destroy(vm_id: u16) -> Result<Interface, Error> {
    call(Interface::NotificationBitmapDestroy { vm_id })
}

pub fn notification_bind(
    sender_id: u16,
    receiver_id: u16,
    flags: NotificationBindFlags,
    bitmap: u64,
) -> Result<Interface, Error> {
    call(Interface::NotificationBind {
        sender_id,
        receiver_id,
        flags,
        bitmap,
    })
}

pub fn notification_unbind(
    sender_id: u16,
    receiver_id: u16,
    bitmap: u64,
) -> Result<Interface, Error> {
    call(Interface::NotificationUnbind {
        sender_id,
        receiver_id,
        bitmap,
    })
}

pub fn notification_set(
    sender_id: u16,
    receiver_id: u16,
    flags: NotificationSetFlags,
    bitmap: u64,
) -> Result<Interface, Error> {
    call(Interface::NotificationSet {
        sender_id,
        receiver_id,
        flags,
        bitmap,
    })
}

pub fn notification_get(
    vcpu_id: u16,
    endpoint_id: u16,
    flags: NotificationGetFlags,
) -> Result<Interface, Error> {
    call(Interface::NotificationGet {
        vcpu_id,
        endpoint_id,
        flags,
    })
}

pub fn notification_info_get(is_32bit: bool) -> Result<Interface, Error> {
    call(Interface::NotificationInfoGet { is_32bit })
}

pub fn mem_perm_get(addr: MemAddr, page_cnt: u32) -> Result<Interface, Error> {
    call(Interface::MemPermGet { addr, page_cnt })
}

pub fn mem_perm_set(
    addr: MemAddr,
    mem_perm: MemPermissionsGetSet,
    page_cnt: u32,
) -> Result<Interface, Error> {
    call(Interface::MemPermSet {
        addr,
        mem_perm,
        page_cnt,
    })
}

pub fn mem_relinquish() -> Result<Interface, Error> {
    call(Interface::MemRelinquish {})
}

pub fn el3_intr_handle() -> Result<Interface, Error> {
    call(Interface::El3IntrHandle)
}

pub fn call(interface: Interface) -> Result<Interface, Error> {
    let regs = call_raw(interface);
    Interface::from_regs(FFA_VERSION, &regs)
}

fn call_raw(interface: Interface) -> [u64; 18] {
    let function_id = u32::from(interface.function_id().unwrap());
    let mut regs = [0; 18];
    interface.to_regs(FFA_VERSION, &mut regs);
    smc64(function_id.into(), regs[1..].try_into().unwrap())
}
