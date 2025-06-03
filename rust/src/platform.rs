// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "fvp")]
mod fvp;
#[cfg(platform = "qemu")]
mod qemu;
#[cfg(test)]
mod test;

use arm_gic::gicv3::GicV3;
use arm_psci::Mpidr;
use core::arch::global_asm;
#[cfg(platform = "fvp")]
pub use fvp::Fvp as PlatformImpl;
#[cfg(not(test))]
pub use percore::exception_free;
#[cfg(platform = "qemu")]
pub use qemu::Qemu as PlatformImpl;
#[cfg(test)]
pub use test::{exception_free, TestPlatform as PlatformImpl};

use crate::{
    context::EntryPointInfo,
    debug::DEBUG,
    gicv3,
    pagetable::IdMap,
    services::{arch::WorkaroundSupport, psci::PsciPlatformInterface},
};

/// The code must use `platform::LoggerWriter` to avoid the 'ambiguous associated type' error that
/// occurs when using `PlatformImpl::LoggerWriter` directly.
pub type LoggerWriter = <PlatformImpl as Platform>::LoggerWriter;

pub type PsciPlatformImpl = <PlatformImpl as Platform>::PsciPlatformImpl;
pub type PlatformPowerState = <PsciPlatformImpl as PsciPlatformInterface>::PlatformPowerState;

unsafe extern "C" {
    /// Given a valid MPIDR value, returns the corresponding linear core index.
    ///
    /// The implementation must never return the same index for two different valid MPIDR values,
    /// and must never return a value greater than or equal to the corresponding
    /// `Platform::CORE_COUNT`.
    ///
    /// For an invalid MPIDR value no guarantees are made about the return value.
    pub safe fn plat_calc_core_pos(mpidr: u64) -> usize;
}

/// The hooks implemented by all platforms.
pub trait Platform {
    /// The number of CPU cores.
    const CORE_COUNT: usize;

    /// The size in bytes of the largest cache line across all the cache levels in the platform.
    const CACHE_WRITEBACK_GRANULE: usize;

    /// The GIC configuration.
    const GIC_CONFIG: gicv3::GicConfig;

    /// The number of pages to reserve for the page heap.
    const PAGE_HEAP_PAGE_COUNT: usize = 5;

    /// Platform dependent Write implementation type for Logger.
    type LoggerWriter: core::fmt::Write;

    /// Platform dependent PsciPlatformInterface implementation type.
    type PsciPlatformImpl: PsciPlatformInterface;

    /// Initialises the logger and anything else the platform needs. This will be called before the
    /// MMU is enabled.
    ///
    /// Any logs sent before this is called will be ignored.
    fn init_before_mmu();

    /// Maps device memory and any other regions specific to the platform, before the MMU is
    /// enabled.
    fn map_extra_regions(idmap: &mut IdMap);

    /// Creates instance of GIC driver.
    ///
    /// # Safety
    ///
    /// This must only be called once, to avoid creating aliases of the GIC driver.
    unsafe fn create_gic() -> GicV3<'static>;

    /// Returns the entry point for the secure world, i.e. BL32.
    fn secure_entry_point() -> EntryPointInfo;

    /// Returns the entry point for the non-secure world, i.e. BL33.
    fn non_secure_entry_point() -> EntryPointInfo;

    /// Returns the entry point for the realm world.
    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo;

    /// Returns whether the given MPIDR is valid for this platform.
    fn mpidr_is_valid(mpidr: Mpidr) -> bool;

    /// Returns an option with a PSCI platform implementation handle. The function should only be
    /// called once, when it returns `Some`. All subsequent calls must return `None`.
    fn psci_platform() -> Option<Self::PsciPlatformImpl>;

    /// Returns whether this platform supports the arch WORKAROUND_1 SMC.
    fn arch_workaround_1_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_1 SMC.
    fn arch_workaround_1();

    /// Returns whether this platform supports the arch WORKAROUND_2 SMC.
    fn arch_workaround_2_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_2 SMC.
    fn arch_workaround_2();

    /// Returns whether this platform supports the arch WORKAROUND_3 SMC.
    fn arch_workaround_3_supported() -> WorkaroundSupport;

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_3 SMC.
    fn arch_workaround_3();

    /// Returns whether this platform supports the arch WORKAROUND_4 SMC.
    fn arch_workaround_4_supported() -> WorkaroundSupport;
}

#[cfg(target_arch = "aarch64")]
global_asm!(
    include_str!("asm_macros_common.S"),
    include_str!("platform_helpers.S"),
    include_str!("asm_macros_common_purge.S"),
    DEBUG = const DEBUG as i32,
);
