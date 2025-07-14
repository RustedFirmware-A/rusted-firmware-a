// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "fvp")]
mod fvp;
#[cfg(platform = "qemu")]
mod qemu;

use arm_gic::gicv3::GicV3;
use arm_sysregs::{MpidrEl1, read_mpidr_el1};
use core::fmt::Write;
use percore::Cores;

#[cfg(platform = "fvp")]
pub type PlatformImpl = fvp::Fvp;
#[cfg(platform = "qemu")]
pub type PlatformImpl = qemu::Qemu;

/// The hooks implemented by each platform.
///
/// # Safety
///
/// `core_position` must be a naked function which doesn't access any memory, and must never return
/// the same index for two different valid MPIDR values. It must only clobber x0-x3.
pub unsafe trait Platform {
    /// The number of CPU cores.
    const CORE_COUNT: usize;

    /// Returns something to which logs should be sent.
    ///
    /// This should only be called once, and may panic on subsequent calls.
    fn make_log_sink() -> &'static mut (dyn Write + Send);

    /// Returns the GIC instance of the platform.
    ///
    /// # Safety
    ///
    /// This must only be called once, to avoid creating aliases of the GIC driver.
    unsafe fn create_gic() -> GicV3<'static>;

    /// Given a valid MPIDR value, returns the corresponding linear core index.
    ///
    /// The implementation must never return the same index for two different valid MPIDR values,
    /// and must never return a value greater than or equal to the corresponding
    /// `Platform::CORE_COUNT`. It must return 0 for the primary core, i.e. the core which powers on
    /// first and handles initialisation.
    ///
    /// For an invalid MPIDR value no guarantees are made about the return value.
    extern "C" fn core_position(mpidr: MpidrEl1) -> usize;

    /// Given a linear core index, returns the corresponding PSCI MPIDR value.
    ///
    /// This is not quite the inverse function of `core_position`, as it doesn't include the MT and
    /// U bits which `core_position` may expect.
    fn psci_mpidr_for_core(core_index: usize) -> u64;
}

// SAFETY: `Platform::core_position` is guaranteed to return a unique value for any valid MPIDR
// value.
unsafe impl Cores for PlatformImpl {
    fn core_index() -> usize {
        Self::core_position(read_mpidr_el1())
    }
}
