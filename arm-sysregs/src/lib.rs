// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Access to Arm CPU system registers.

#![cfg_attr(not(any(test, feature = "fakes")), no_std)]

#[cfg(not(any(test, feature = "fakes")))]
mod aarch64;
#[cfg(any(test, feature = "fakes"))]
pub mod fake;

/// Generates public functions named `$read_function_name` and `$write_function_name` to read or
/// write (respectively) a value of type `$type` from/to the system register `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to read from or write
/// any value to.
#[macro_export]
macro_rules! read_write_sysreg {
    ($sysreg:ident, $type:ty $(: $bitflags_type:ty)?, safe $read_function_name:ident, safe $write_function_name:ident $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $read_function_name $(, $fake_sysregs)?);
        $crate::write_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $write_function_name $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty $(: $bitflags_type:ty)?, safe $read_function_name:ident, $write_function_name:ident $(, $fake_sysregs:expr)?
    ) => {
        $crate::read_sysreg!($sysreg, $type $(: $bitflags_type)?, safe $read_function_name $(, $fake_sysregs)?);
        $crate::write_sysreg! {
            $(#[$attributes])*
            $sysreg, $type $(: $bitflags_type)?, $write_function_name $(, $fake_sysregs)?
        }
    };
}
