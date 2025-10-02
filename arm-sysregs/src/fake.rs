// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake implementations of system register getters and setters for unit tests.

use crate::MpidrEl1;
use std::sync::Mutex;

/// Generates a public function named `read_$sysreg` to read the fake system register `$sysreg` of
/// type `$type`.
#[macro_export]
macro_rules! read_sysreg {
    ($sysreg:ident, $type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< read_ $sysreg >]() -> $type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< read_ $sysreg >]() -> $type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< read_ $sysreg >]() -> $bitflags_type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty : $bitflags_type:ty, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< read_ $sysreg >]() -> $bitflags_type {
                $fake_sysregs.lock().unwrap().$sysreg
            }
        }
    };
}

/// Generates a public function named `write_$sysreg` to write to the fake system register `$sysreg`
/// of type `$type`.
#[macro_export]
macro_rules! write_sysreg {
    ($sysreg:ident, $type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< write_ $sysreg >](value: $type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty, $fake_sysregs:expr
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< write_ $sysreg >](value: $type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe, $fake_sysregs:expr) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            pub fn [< write_ $sysreg >](value: $bitflags_type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty : $bitflags_type:ty, $fake_sysregs:expr
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            pub unsafe fn [< write_ $sysreg >](value: $bitflags_type) {
                $fake_sysregs.lock().unwrap().$sysreg = value;
            }
        }
    };
}

/// Values of fake system registers.
pub static SYSREGS: Mutex<SystemRegisters> = Mutex::new(SystemRegisters::new());

/// A set of fake system registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRegisters {
    /// Fake value for the MPIDR_EL1 system register.
    pub mpidr_el1: MpidrEl1,
}

impl SystemRegisters {
    const fn new() -> Self {
        Self {
            mpidr_el1: MpidrEl1::empty(),
        }
    }

    /// Resets the fake system registers to their initial state.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
