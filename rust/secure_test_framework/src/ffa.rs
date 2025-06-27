// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_ffa::{DirectMsgArgs, Error, Interface, MsgWaitFlags, Version};
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

pub fn msg_wait(flags: Option<MsgWaitFlags>) -> Result<Interface, Error> {
    call(Interface::MsgWait { flags })
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

fn call(interface: Interface) -> Result<Interface, Error> {
    let regs = call_raw(interface);
    Interface::from_regs(FFA_VERSION, &regs)
}

fn call_raw(interface: Interface) -> [u64; 18] {
    let function_id = u32::from(interface.function_id().unwrap());
    let mut regs = [0; 18];
    interface.to_regs(FFA_VERSION, &mut regs);
    smc64(function_id.into(), regs[1..].try_into().unwrap())
}
