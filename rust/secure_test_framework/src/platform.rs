// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "qemu")]
mod qemu;

use core::fmt::Write;

#[cfg(platform = "qemu")]
pub type PlatformImpl = qemu::Qemu;

/// The hooks implemented by each platform.
pub trait Platform {
    /// Returns something to which logs should be sent.
    ///
    /// This should only be called once, and may panic on subsequent calls.
    fn make_log_sink() -> &'static mut (dyn Write + Send);
}
