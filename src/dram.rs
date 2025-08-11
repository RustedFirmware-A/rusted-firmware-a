// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Safe abstractions for storing data in DRAM sections.

use zerocopy::FromZeros;

/// Creates a zeroed instance of the given type.
///
/// This is equivalent to `FromZeroes::new_zeroed()` but const.
pub const fn const_zeroed<T: FromZeros>() -> T {
    // SAFETY: T implements `FromZeros` so it must be safe to initialise with zeros.
    unsafe { core::mem::zeroed() }
}

/// Declares a static zero-initialised `$t`, and a SpinMutex initialised with a mutable reference to
/// it. E.g.,
///
/// ```
/// zeroed_mut!(FOO, u64);
/// ```
///
/// will create
///
/// ```
/// static FOO: SpinMutex<&'static mut u64> = ...;
/// ```
///
/// For this to work, `$t` must implement `zerocopy::FromZeroes`.
///
/// Attributes can optionally be provided both for the underlying static and for the `SpinMutex`
/// wrapper, e.g.:
///
/// ```
/// zeroed_mut! {
///     /// Rustdoc comment for FOO.
///     pub FOO, u64, unsafe(link_section = ".bss.dram")
/// }
/// ```
#[allow(unused_macros)]
macro_rules! zeroed_mut {
    ($(#[$attributes:meta])* $visibility:vis $name:ident, $t:ty $(, $raw_attributes:meta)*) => {
        $(#[$attributes])*
        $visibility static $name: spin::mutex::SpinMutex<&'static mut $t> = spin::mutex::SpinMutex::new({
            $(#[$raw_attributes])*
            static mut RAW: $t = $crate::dram::const_zeroed();
            // SAFETY: This is the only place where we create a reference to the contents of this
            // static mut.
            unsafe { &mut *&raw mut RAW }
        });
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_zeroed() {
        static TEST_ZEROED: [u8; 100] = const_zeroed();

        assert_eq!(TEST_ZEROED[0], 0);
        assert_eq!(TEST_ZEROED[99], 0);
    }

    #[test]
    fn mutable_macro() {
        zeroed_mut!(TEST, u64);

        let mut test_ref = TEST.lock();
        assert_eq!(**test_ref, 0);
        **test_ref = 42;
        assert_eq!(**test_ref, 42);
    }
}
