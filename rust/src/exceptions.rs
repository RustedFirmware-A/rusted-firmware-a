// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::CpuContext,
    services::{arch, psci},
    smccc::{FunctionId, SmcccCallType, NOT_SUPPORTED},
};
use bitflags::bitflags;
use core::{ffi::c_void, ptr::null_mut};
use log::debug;

const TRAP_RET_UNHANDLED: i64 = -1;

#[no_mangle]
extern "C" fn handle_sysreg_trap(_esr_el3: u64, _ctx: *mut CpuContext) -> i64 {
    TRAP_RET_UNHANDLED
}

/// Returns the type of the highest priority pending interrupt at the interrupt controller.
#[no_mangle]
extern "C" fn plat_ic_get_pending_interrupt_type() -> u32 {
    unimplemented!();
}

#[no_mangle]
extern "C" fn get_interrupt_type_handler(_interrupt_type: u32) -> *mut c_void {
    null_mut()
}

/// Handler for injecting undefined exception to lower EL caused by the lower EL accessing system
/// registers of which EL3 firmware is unaware.
///
/// This is a safety net to avoid EL3 panics caused by system register access.
///
/// # Safety
///
/// The `ctx` must be a valid pointer to a `CpuContext` with live aliases for the duration of the
/// function call.
#[no_mangle]
unsafe extern "C" fn inject_undef64(_ctx: *mut CpuContext) {
    unimplemented!();
}

bitflags! {
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct SmcFlags: u64 {
        const NON_SECURE = 1 << 0;
        const REALM = 1 << 5;
        const SVE_HINT = 1 << 16;
    }
}

/// Called from the exception handler in assembly to handle an SMC.
///
/// # Safety
///
/// `context` must be a valid CpuContext for some world on the current CPU core.
#[no_mangle]
unsafe extern "C" fn handle_smc(
    function: FunctionId,
    x1: u64,
    x2: u64,
    x3: u64,
    x4: u64,
    context: *mut CpuContext,
    flags: SmcFlags,
) -> *mut CpuContext {
    debug!(
        "Handling SMC {:?} ({:#0x}, {:#0x}, {:#0x}, {:#0x}) with context {:?}, flags {:?}",
        function, x1, x2, x3, x4, context, flags
    );
    let ret = match (function.call_type(), function.oen()) {
        (SmcccCallType::Fast32 | SmcccCallType::Fast64, arch::OEN) => {
            arch::handle_smc(function, x1, x2, x3, x4, flags)
        }
        (SmcccCallType::Fast32 | SmcccCallType::Fast64, psci::OEN) => {
            psci::handle_smc(function, x1, x2, x3, x4, flags)
        }
        _ => NOT_SUPPORTED.into(),
    };

    unsafe {
        // TODO: Get `CpuContext` from `CPU_STATE` safely based on current world? The `context`
        // passed here may alias one obtained from `CPU_STATE` somewhere else.
        (*context).gpregs.write_return_value(&ret);
    }
    context
}
