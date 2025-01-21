// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod arch;
pub mod psci;

use self::{arch::Arch, psci::Psci};
use crate::{
    context::World,
    smccc::{FunctionId, SmcReturn, NOT_SUPPORTED},
};

/// Helper macro to define the range of SMC function ID values covered by a service
#[macro_export]
macro_rules! owns {
    // service handles the entire Owning Entity Number (OEN)
    ($owning_entity:expr) => {
        #[inline(always)]
        fn owns(function: FunctionId) -> bool {
            function.oen() == $owning_entity
                && matches!(
                    function.call_type(),
                    crate::smccc::SmcccCallType::Fast32 | crate::smccc::SmcccCallType::Fast64
                )
        }
    };
    // service handles a sub-range of the OEN
    // range refers to the lower 16 bits [15:0] of the SMC FunctionId
    ($owning_entity:expr, $range:expr) => {
        #[inline(always)]
        fn owns(function: FunctionId) -> bool {
            function.oen() == $owning_entity
                && $range.contains(&function.number())
                && matches!(
                    function.call_type(),
                    crate::smccc::SmcccCallType::Fast32 | crate::smccc::SmcccCallType::Fast64
                )
        }
    };
}
pub(crate) use owns;

/// A service which handles some range of SMC calls.
pub trait Service {
    /// Returns whether this service is intended to handle the given function ID.
    fn owns(function: FunctionId) -> bool;

    /// Handles the given SMC call.
    fn handle_smc(
        function: FunctionId,
        x1: u64,
        x2: u64,
        x3: u64,
        x4: u64,
        world: World,
    ) -> SmcReturn;
}

/// Calls the appropriate SMC handler based on the function ID, or returns `NOT_SUPPORTED` if there
/// is no suitable handler.
pub fn dispatch_smc(
    function: FunctionId,
    x1: u64,
    x2: u64,
    x3: u64,
    x4: u64,
    world: World,
) -> SmcReturn {
    if Arch::owns(function) {
        Arch::handle_smc(function, x1, x2, x3, x4, world)
    } else if Psci::owns(function) {
        Psci::handle_smc(function, x1, x2, x3, x4, world)
    } else {
        NOT_SUPPORTED.into()
    }
}
