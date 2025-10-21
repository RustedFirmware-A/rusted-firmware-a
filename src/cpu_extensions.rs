// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! A framework for managing ARM architectural CPU extensions using a trait-based approach.

pub mod hcx;
pub mod sys_reg_trace;
pub mod trf;

use crate::{
    context::{CpuContext, PerWorldContext, World},
    platform::{Platform, PlatformImpl},
};

/// A trait for managing CPU extensions.
pub trait CpuExtension {
    /// Checks if the CPU extension is supported by the hardware.
    fn is_present(&self) -> bool;

    /// Optional function to enable the feature in-place in any EL3 registers that are never
    /// context switched.
    /// The values written must never change.
    fn init(&self) {}

    /// Configures the per-world EL3 registers to enable this extension.
    /// TODO: Switch to const traits when it becomes a stable feature:
    /// <https://github.com/rust-lang/rust/issues/143874>
    fn configure_per_world(&self, _world: World, _ctx: &mut PerWorldContext) {}

    /// Configures the per-cpu EL3 registers related to this extension.
    fn configure_per_cpu(&self, _world: World, _context: &mut CpuContext) {}

    /// Save the extension-specific registers before switching from world `world`.
    ///
    /// If an extension needs to save and restore any context, this function is responsible for
    /// checking if the extension is supported by the hardware. This way `save_context` will be a
    /// no-op for every extension that does not have any context.
    fn save_context(&self, _world: World) {}

    /// Restore the extension-specific registers after switching to world `world`.
    ///
    /// If an extension needs to save and restore any context, this function is responsible for
    /// checking if the extension is supported by the hardware. This way `restore_context` will be
    /// a no-op for every extension that does not have any context.
    fn restore_context(&self, _world: World) {}
}

/// Enable architecture extensions for EL3 execution. This function only updates
/// registers in-place which are expected to either never change or be
/// overwritten by el3_exit.
pub fn initialise_el3_sysregs() {
    for ext in PlatformImpl::CPU_EXTENSIONS {
        if ext.is_present() {
            ext.init();
        }
    }
    // TODO: initialize PMU
}
