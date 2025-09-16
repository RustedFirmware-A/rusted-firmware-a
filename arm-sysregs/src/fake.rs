// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake implementations of system register getters and setters for unit tests.

/// Generates a public function named `$function_name` to read the fake system register `$sysreg` of
/// type `$type`.
#[macro_export]
macro_rules! read_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Returns the value of the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        pub fn $function_name() -> $type {
            $fake_sysregs.lock().unwrap().$sysreg
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty, $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Returns the value of the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        $(#[$attributes])*
        pub unsafe fn $function_name() -> $type {
            $fake_sysregs.lock().unwrap().$sysreg
        }
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Returns the value of the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        pub fn $function_name() -> $bitflags_type {
            $fake_sysregs.lock().unwrap().$sysreg
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty : $bitflags_type:ty, $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Returns the value of the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        $(#[$attributes])*
        pub unsafe fn $function_name() -> $bitflags_type {
            $fake_sysregs.lock().unwrap().$sysreg
        }
    };
}

/// Generates a public function named `$function_name` to write to the fake system register
/// `$sysreg` of type `$type`.
#[macro_export]
macro_rules! write_sysreg {
    ($sysreg:ident, $type:ty, safe $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Writes `value` to the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        pub fn $function_name(value: $type) {
            $fake_sysregs.lock().unwrap().$sysreg = value;
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty, $function_name:ident, $fake_sysregs:expr
    ) => {
        #[doc = "Writes `value` to the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $type) {
            $fake_sysregs.lock().unwrap().$sysreg = value;
        }
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe $function_name:ident, $fake_sysregs:expr) => {
        #[doc = "Writes `value` to the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        pub fn $function_name(value: $bitflags_type) {
            $fake_sysregs.lock().unwrap().$sysreg = value;
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty : $bitflags_type:ty, $function_name:ident, $fake_sysregs:expr
    ) => {
        #[doc = "Writes `value` to the `"]
        #[doc = stringify!($sysreg)]
        #[doc = "` system register."]
        $(#[$attributes])*
        pub unsafe fn $function_name(value: $bitflags_type) {
            $fake_sysregs.lock().unwrap().$sysreg = value;
        }
    };
}
