// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// Generates public functions named `read_$sysreg` and `write_$sysreg` to read or write
/// (respectively) a value of type `$type` from/to the system register `$sysreg`.
///
/// `safe_read` and `safe_write` should only be specified for system registers which are indeed safe
/// to read from or write any value to.
#[macro_export]
macro_rules! read_write_sysreg {
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty $(: $bitflags_type:ty)?, safe_read, safe_write $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
        $crate::write_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty $(: $bitflags_type:ty)?, safe_read $(, $fake_sysregs:expr)?
    ) => {
        $crate::read_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
        $crate::write_sysreg! {
            $(#[$attributes])*
            $sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)? $(, $fake_sysregs)?
        }
    };
}
