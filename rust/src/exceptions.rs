// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{cpu_state, World},
    platform::exception_free,
    services::dispatch_smc,
    smccc::FunctionId,
};
use bitflags::bitflags;
use core::{ffi::c_void, ptr::null_mut};
use log::debug;

const TRAP_RET_UNHANDLED: i64 = -1;

#[unsafe(no_mangle)]
extern "C" fn handle_sysreg_trap(_esr_el3: u64, _world: World) -> i64 {
    TRAP_RET_UNHANDLED
}

/// Returns the type of the highest priority pending interrupt at the interrupt controller.
#[unsafe(no_mangle)]
extern "C" fn plat_ic_get_pending_interrupt_type() -> u32 {
    unimplemented!();
}

#[unsafe(no_mangle)]
extern "C" fn get_interrupt_type_handler(_interrupt_type: u32) -> *mut c_void {
    null_mut()
}

/// Handler for injecting undefined exception to lower EL caused by the lower EL accessing system
/// registers of which EL3 firmware is unaware.
///
/// This is a safety net to avoid EL3 panics caused by system register access.
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
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

    let ret = dispatch_smc(function, x1, x2, x3, x4, world);

    // Write the return value back to the registers of the world that made the SMC call. Note that
    // this might not be the same world as we are about to return to, as the handler might have
    // switched worlds by calling `set_next_world_context`.
    exception_free(|token| {
        cpu_state(token)
            .context_mut(world)
            .gpregs
            .write_return_value(&ret);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::arch::{SMCCC_VERSION, SMCCC_VERSION_1_5};

    /// Tests the SMCCC arch version call as a simple example of SMC dispatch.
    ///
    /// The point of this isn't to test every individual SMC call, just that the common code in
    /// `handle_smc` works. Individual SMC calls can be tested directly within their modules.
    #[test]
    fn handle_smc_arch_version() {
        handle_smc(
            FunctionId(SMCCC_VERSION),
            0,
            0,
            0,
            0,
            World::NonSecure,
            SmcFlags::empty(),
        );

        assert_eq!(
            exception_free(|token| { cpu_state(token).context(World::NonSecure).gpregs.registers }),
            [
                SMCCC_VERSION_1_5 as u64,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]
        );
    }
}
