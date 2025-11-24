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
pub const SECURE_WORLD_ID: u16 = 0x8001;

/// Default ID for the SPMC
pub const SPMC_DEFAULT_ID: u16 = 0x8000;

/// Default ID for the SPMD
#[allow(unused)]
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

/// Enables PAuth at the current exception level using the provided key.
#[cfg(feature = "pauth")]
pub fn enable_pauth(key: u128) {
    use arm_sysregs::{
        ApiakeyhiEl1, ApiakeyloEl1, SctlrEl1, SctlrEl2, read_sctlr_el1, read_sctlr_el2,
        write_apiakeyhi_el1, write_apiakeylo_el1, write_sctlr_el1, write_sctlr_el2,
    };

    unsafe {
        write_apiakeylo_el1(ApiakeyloEl1::from_bits_retain(key as u64));
        write_apiakeyhi_el1(ApiakeyhiEl1::from_bits_retain((key >> 64) as u64));
    }
    if current_el() == 2 {
        // SAFETY: We have set the PAuth key, so it is safe to enable PAuth.
        unsafe {
            write_sctlr_el2(read_sctlr_el2() | SctlrEl2::ENIA);
        }
    } else {
        // SAFETY: We have set the PAuth key, so it is safe to enable PAuth.
        unsafe {
            write_sctlr_el1(read_sctlr_el1() | SctlrEl1::ENIA);
        }
    }
    // SAFETY: The `isb` instruction does not violate safe Rust guarantees.
    unsafe {
        asm!("isb", options(nostack));
    }
}

/// If the result contains an error then prints it along with the given message, and returns an
/// empty tuple error.
///
/// This is convenient for handling errors which should cause a test to fail.
pub fn log_error<V, E: Display>(message: &str, result: Result<V, E>) -> Result<V, ()> {
    result.map_err(|e| error!("{message}: {e}"))
}

/// If the given FF-A response is a success then returns its arguments, otherwise logs and returns
/// an error.
pub fn expect_ffa_success(response: Interface) -> Result<SuccessArgs, ()> {
    if let Interface::Success { args, .. } = response {
        Ok(args)
    } else {
        error!("Expected success but got {response:?}");
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
        error!("Expected MemRetrieveResp but got {response:?}");
        Err(())
    }
}

/// Triggers a SMC call with the given function/interface, checks that this call was successful (logs an error
/// otherwise) and checks whether the response's interface matches the expected one.
macro_rules! expect_ffa_interface {
    ($expect:ident, $message:expr, $call:expr) => {
        $expect(crate::util::log_error($message, $call)?)?
    };
}
pub(crate) use expect_ffa_interface;

/// This macro wraps a naked_asm block with `bti`, or any other universal
/// prologue we'd still like added.
///
/// Use this over `core::arch::naked_asm` by default, otherwise you may
/// need to ensure that e.g. `bti` landing pads are in place yourself.
macro_rules! naked_asm {
    ($($inner:tt)*) => {
       ::core::arch::naked_asm!("bti c", $($inner)*)
    }
}

pub(crate) use naked_asm;
