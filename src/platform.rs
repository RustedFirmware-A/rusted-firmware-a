// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

macro_rules! select_platform {
    (platform = $condition:literal, $mod:ident::$sub:ident::$plat_impl:ident) => {
        #[cfg(platform = $condition)]
        mod $mod;

        #[cfg(platform = $condition)]
        pub use $mod::$sub::{CPU_OPS, $plat_impl as PlatformImpl};
    };
    (platform = $condition:literal, $mod:ident::$plat_impl:ident) => {
        #[cfg(platform = $condition)]
        mod $mod;

        #[cfg(platform = $condition)]
        pub use $mod::{CPU_OPS, $plat_impl as PlatformImpl};
    };
    (test, $mod:ident::$plat_impl:ident) => {
        #[cfg(test)]
        mod $mod;

        #[cfg(test)]
        pub use $mod::{CPU_OPS, $plat_impl as PlatformImpl};
    };
}

select_platform!(platform = "fvp", fvp::Fvp);
select_platform!(platform = "qemu", qemu::Qemu);
select_platform!(test, test::TestPlatform);

use crate::{
    context::EntryPointInfo,
    gicv3,
    logger::LogSink,
    pagetable::IdMap,
    services::{
        Service, arch::WorkaroundSupport, psci::PsciPlatformInterface, trng::TrngPlatformInterface,
    },
    smccc::FunctionId,
    sysregs::MpidrEl1,
};
use arm_gic::{IntId, gicv3::GicV3};
#[cfg(not(test))]
pub use percore::exception_free;
#[cfg(test)]
pub use test::exception_free;

/// For platforms that do not want to implement any custom SMC handlers.
pub struct DummyService;

impl Service for DummyService {
    fn owns(&self, _function: FunctionId) -> bool {
        // Does not own any function id.
        false
    }
}

/// Type alias for convenience, to avoid having to use the complicated type name everywhere.
pub type LogSinkImpl = <PlatformImpl as Platform>::LogSinkImpl;

pub type PsciPlatformImpl = <PlatformImpl as Platform>::PsciPlatformImpl;
pub type TrngPlatformImpl = <PlatformImpl as Platform>::TrngPlatformImpl;
pub type PlatformPowerState = <PsciPlatformImpl as PsciPlatformInterface>::PlatformPowerState;

pub type PlatformServiceImpl = <PlatformImpl as Platform>::PlatformServiceImpl;

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

    /// Platform dependent LogSink implementation type for Logger.
    type LogSinkImpl: LogSink;

    /// Platform dependent PsciPlatformInterface implementation type.
    type PsciPlatformImpl: PsciPlatformInterface;

    /// Platform dependent TrngPlatformInterface implementation type.
    type TrngPlatformImpl: TrngPlatformInterface;

    /// Service that handles platform-specific SMC calls.
    type PlatformServiceImpl: Service;

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

    /// Creates instance of PlatformServiceImpl.
    ///
    /// This is used for dispatching platform-specific SMCs.
    ///
    /// TODO: provide default implementation with DummyService
    /// once associated type defaults become stable
    /// see issue #29661 <https://github.com/rust-lang/rust/issues/29661>
    fn create_service() -> Self::PlatformServiceImpl;

    /// Handles a Group 0 interrupt.
    ///
    /// Interrupt with id `int_id` has already been acknowledged at this point
    /// and platform-independent code will set EOI after this function returns.
    fn handle_group0_interrupt(int_id: IntId);

    /// Returns the entry point for the secure world, i.e. BL32.
    fn secure_entry_point() -> EntryPointInfo;

    /// Returns the entry point for the non-secure world, i.e. BL33.
    fn non_secure_entry_point() -> EntryPointInfo;

    /// Returns the entry point for the realm world.
    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo;

    /// Returns whether the given MPIDR is valid for this platform.
    fn mpidr_is_valid(mpidr: MpidrEl1) -> bool;

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

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use crate::debug::DEBUG;
    use core::arch::global_asm;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("platform_helpers.S"),
        include_str!("asm_macros_common_purge.S"),
        DEBUG = const DEBUG as i32,
    );
}
