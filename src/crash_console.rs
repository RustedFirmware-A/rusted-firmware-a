// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Crash console drivers.

#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
pub mod pl011;

/// Crash console platform interface
///
/// # Safety
///
/// The implementations of `crash_console_init`, `crash_console_putc` and `crash_console_flush` must
/// be naked functions which don't use the stack, and only clobber the registers they are documented
/// to clobber. Test implementations are an exception, where an empty non-naked implementation is
/// sufficient.
pub unsafe trait CrashConsole {
    /// Initialises the crash console to print a crash report.
    ///
    /// This may be called without a Rust runtime, e.g. with no stack.
    ///
    /// It must use only x0-x4 and return 1 on success in x0.
    #[cfg_attr(test, allow(unused))]
    extern "C" fn crash_console_init() -> u32;

    /// Prints a character on the crash console.
    ///
    /// This may be called without a Rust runtime, e.g. with no stack.
    ///
    /// May clobber x1-x2.
    #[cfg_attr(test, allow(unused))]
    extern "C" fn crash_console_putc(char: u32) -> i32;

    /// Forces a write of all buffered data that hasn't been output.
    ///
    /// This may be called without a Rust runtime, e.g. with no stack.
    ///
    /// May clobber x0-x5.
    #[cfg_attr(test, allow(unused))]
    extern "C" fn crash_console_flush();
}
