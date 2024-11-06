// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{cpu_state, World},
    services::{arch, psci},
    smccc::{FunctionId, SmcccCallType, NOT_SUPPORTED},
};
use bitflags::bitflags;
use core::{ffi::c_void, ptr::null_mut};
use log::debug;
use percore::exception_free;

const TRAP_RET_UNHANDLED: i64 = -1;

#[no_mangle]
extern "C" fn handle_sysreg_trap(_esr_el3: u64, _world: World) -> i64 {
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
#[no_mangle]
extern "C" fn inject_undef64(_world: World) {
    unimplemented!();
}

bitflags! {
    // These bit flags must match those set in `runtime_exceptions.S`.
    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct SmcFlags: u64 {
        const NON_SECURE = 1 << 0;
        const REALM = 1 << 5;
        const SVE_HINT = 1 << 16;
    }
}

/// Called from the exception handler in assembly to handle an SMC.
#[no_mangle]
extern "C" fn handle_smc(
    function: FunctionId,
    x1: u64,
    x2: u64,
    x3: u64,
    x4: u64,
    world: World,
    flags: SmcFlags,
) {
    debug!(
        "Handling SMC {:?} ({:#0x}, {:#0x}, {:#0x}, {:#0x}) with world {:?}, flags {:?}",
        function, x1, x2, x3, x4, world, flags
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

    // Write the return value back to the registers of the world that made the SMC call. Note that
    // this might not be the same world as we are about to return to, as the handler might have
    // switched worlds by calling `set_next_world_context`.
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        cpu_state.context_mut(world).gpregs.write_return_value(&ret);
    });
}
