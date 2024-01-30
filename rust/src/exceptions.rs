// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::context::CpuContext;
use core::{ffi::c_void, ptr::null_mut};

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
