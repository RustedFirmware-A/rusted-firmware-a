// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod arch;
pub mod psci;

/// Helper macro to define the range of SMC function ID values covered by a service
#[macro_export]
macro_rules! owns {
    // service handles the entire Owning Entity Number (OEN)
    ($owning_entity:expr) => {
        #[inline(always)]
        pub fn owns(function: FunctionId) -> bool {
            function.oen().oe() == $owning_entity
                && matches!(
                    function.call_type(),
                    SmcccCallType::Fast32 | SmcccCallType::Fast64
                )
        }
    };
    // service handles a sub-range of the OEN
    // range refers to the lower 16 bits [15:0] of the SMC FunctionId
    ($owning_entity:expr, $range:expr) => {
        use core::ops::RangeInclusive;
        #[inline(always)]
        pub fn owns(function: FunctionId) -> bool {
            function.oen().oe() == $owning_entity
                && $range.contains(&function.number())
                && matches!(
                    function.call_type(),
                    SmcccCallType::Fast32 | SmcccCallType::Fast64
                )
        }
    };
}
pub(crate) use owns;
