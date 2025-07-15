// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod timer;

use arm_ffa::{Interface, SuccessArgs};
use core::{arch::asm, fmt::Display};
use log::error;

/// The partition ID of the normal world component.
pub const NORMAL_WORLD_ID: u16 = 0;

/// The partition ID of the secure world component.
pub const SECURE_WORLD_ID: u16 = 1;

/// Default ID for the SPMC
pub const SPMC_DEFAULT_ID: u16 = 0x8000;

/// Default ID for the SPMD
pub const SPMD_DEFAULT_ID: u16 = 0xffff;

/// Returns the current exception level at which we are running.
pub fn current_el() -> u8 {
    let current_el: u64;
    // SAFETY: Reading `CurrentEL` is always safe, it has no impact on memory.
    unsafe {
        asm!(
            "mrs {current_el}, CurrentEL",
            options(nostack),
            current_el = out(reg) current_el,
        );
    }
    (current_el >> 2) as u8
}

/// If the result contains an error then prints it along with the given message, and returns an
/// empty tuple error.
///
/// This is convenient for handling errors which should cause a test to fail.
pub fn log_error<V, E: Display>(message: &str, result: Result<V, E>) -> Result<V, ()> {
    result.map_err(|e| error!("{}: {}", message, e))
}

/// If the given FF-A response is a success then returns its arguments, otherwise logs and returns
/// an error.
pub fn expect_ffa_success(response: Interface) -> Result<SuccessArgs, ()> {
    if let Interface::Success { args, .. } = response {
        Ok(args)
    } else {
        error!("Expected success but got {:?}", response);
        Err(())
    }
}

/// If the given FF-A response is a mem retrieve response then returns its arguments, otherwise logs
/// and returns an error.
pub fn expect_ffa_mem_retrieve_resp(response: Interface) -> Result<(u32, u32), ()> {
    if let Interface::MemRetrieveResp {
        total_len,
        frag_len,
    } = response
    {
        Ok((total_len, frag_len))
    } else {
        error!("Expected MemRetrieveResp but got {:?}", response);
        Err(())
    }
}

/// Triggers a SMC call with the given function/interface, checks that this call was successful (logs an error
/// otherwise) and checks whether the response's interface matches the expected one.
#[macro_export]
macro_rules! expect_ffa_interface {
    ($expect:ident, $message:expr, $call:expr) => {
        $expect(crate::util::log_error($message, $call)?)?
    };
}
