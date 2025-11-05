// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// Generates a public function named `read_$ident` to read the system register `$sysreg` as a value
/// of type `$type`.
///
/// `safe` should only be specified for system registers which are indeed safe to read.
#[cfg(not(any(test, feature = "fakes")))]
#[macro_export]
macro_rules! read_sysreg {
    ($sysreg:ident : $asm_sysreg:ident, $type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            #[inline(always)]
            pub fn [< read_ $sysreg >]() -> $type {
                let value;
                // SAFETY: The macro call site's author (i.e. see below) has determined that it is
                // always safe to read the given `$sysreg.`
                unsafe {
                    core::arch::asm!(
                        concat!("mrs {value}, ", stringify!($asm_sysreg)),
                        options(nostack),
                        value = out(reg) value,
                    );
                }
                value
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident : $asm_sysreg:ident, $type:ty $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            #[inline(always)]
            pub unsafe fn [< read_ $sysreg >]() -> $type {
                let value;
                // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
                unsafe {
                    core::arch::asm!(
                        concat!("mrs {value}, ", stringify!($asm_sysreg)),
                        options(nostack),
                        value = out(reg) value,
                    );
                }
                value
            }
        }
    };
    ($sysreg:ident : $asm_sysreg:ident, $type:ty : $bitflags_type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            #[inline(always)]
            pub fn [< read_ $sysreg >]() -> $bitflags_type {
                let value: $type;
                // SAFETY: The macro call site's author (i.e. see below) has determined that it is
                // always safe to read the given `$sysreg.`
                unsafe {
                    core::arch::asm!(
                        concat!("mrs {value}, ", stringify!($asm_sysreg)),
                        options(nostack),
                        value = out(reg) value,
                    );
                }
                <$bitflags_type>::from_bits_retain(value)
            }
        }
    };
    ($(#[$attributes:meta])* $sysreg:ident : $asm_sysreg:ident, $type:ty : $bitflags_type:ty $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Returns the value of the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            #[inline(always)]
            pub unsafe fn [< read_ $sysreg >]() -> $bitflags_type {
                let value: $type;
                // SAFETY: The caller promises that it is safe to read the given `$sysreg`.
                unsafe {
                    core::arch::asm!(
                        concat!("mrs {value}, ", stringify!($asm_sysreg)),
                        options(nostack),
                        value = out(reg) value,
                    );
                }
                <$bitflags_type>::from_bits_retain(value)
            }
        }
    };
    ($sysreg:ident, $type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg : $sysreg, $type, safe $(, $fake_sysregs)?);
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($(#[$attributes])* $sysreg : $sysreg, $type $(, $fake_sysregs)?);
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg : $sysreg, $type : $bitflags_type, safe $(, $fake_sysregs)?);
    };
    ($(#[$attributes:meta])* $sysreg:ident, $type:ty : $bitflags_type:ty $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg($(#[$attributes])* $sysreg : $sysreg, $type : $bitflags_type $(, $fake_sysregs)?);
    };
}

/// Generates a public function named `write_$sysreg` to write a value of type `$type` to the system
/// register `$sysreg`.
///
/// `safe` should only be specified for system registers which are indeed safe to write any value
/// to.
#[cfg(not(any(test, feature = "fakes")))]
#[macro_export]
macro_rules! write_sysreg {
    ($sysreg:ident : $asm_sysreg:ident, $type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            #[inline(always)]
            pub fn [< write_ $sysreg >](value: $type) {
                // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
                // to write any value to the given `$sysreg.`
                unsafe {
                    core::arch::asm!(
                        concat!("msr ", stringify!($asm_sysreg), ", {value}"),
                        options(nostack),
                        value = in(reg) value,
                    );
                }
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident : $asm_sysreg:ident, $type:ty $(, $fake_sysregs:expr)?
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            #[inline(always)]
            pub unsafe fn [< write_ $sysreg >](value: $type) {
                // SAFETY: The caller promises that it is safe to write `value` to the given `$sysreg`.
                unsafe {
                    core::arch::asm!(
                        concat!("msr ", stringify!($asm_sysreg), ", {value}"),
                        options(nostack),
                        value = in(reg) value,
                    );
                }
            }
        }
    };
    ($sysreg:ident : $asm_sysreg:ident, $type:ty : $bitflags_type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            #[inline(always)]
            pub fn [< write_ $sysreg >](value: $bitflags_type) {
                let value: $type = value.bits();
                // SAFETY: The macro call site's author (i.e. see below) has determined that it is safe
                // to write any value to the given `$sysreg.`
                unsafe {
                    core::arch::asm!(
                        concat!("msr ", stringify!($asm_sysreg), ", {value}"),
                        options(nostack),
                        value = in(reg) value,
                    );
                }
            }
        }
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident : $asm_sysreg:ident, $type:ty : $bitflags_type:ty $(, $fake_sysregs:expr)?
    ) => {
        $crate::_paste::paste! {
            #[doc = "Writes `value` to the `"]
            #[doc = stringify!($sysreg)]
            #[doc = "` system register."]
            $(#[$attributes])*
            #[inline(always)]
            pub unsafe fn [< write_ $sysreg >](value: $bitflags_type) {
                let value: $type = value.bits();
                // SAFETY: The caller promises that it is safe to write `value` to the given `$sysreg`.
                unsafe {
                    core::arch::asm!(
                        concat!("msr ", stringify!($asm_sysreg), ", {value}"),
                        options(nostack),
                        value = in(reg) value,
                    );
                }
            }
        }
    };
    ($sysreg:ident, $type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::write_sysreg!($sysreg : $sysreg, $type, safe $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty $(, $fake_sysregs:expr)?
    ) => {
        $crate::write_sysreg!($(#[$attributes])* $sysreg : $sysreg, $type $(, $fake_sysregs)?);
    };
    ($sysreg:ident, $type:ty : $bitflags_type:ty, safe $(, $fake_sysregs:expr)?) => {
        $crate::write_sysreg!($sysreg : $sysreg, $type : $bitflags_type, safe $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident, $type:ty : $bitflags_type:ty $(, $fake_sysregs:expr)?
    ) => {
        $crate::write_sysreg!($(#[$attributes])* $sysreg : $sysreg, $type : $bitflags_type $(, $fake_sysregs)?);
    };
}
