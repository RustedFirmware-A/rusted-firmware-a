// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RF-A: A new implementation of TF-A for AArch64.

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]

mod aarch64;
mod context;
mod cpu;
mod cpu_extensions;
#[cfg(not(test))]
mod crash_console;
mod debug;
mod dram;
mod errata_framework;
mod exceptions;
mod gicv3;
#[cfg(feature = "rme")]
mod gpt;
#[cfg_attr(test, path = "layout_fake.rs")]
mod layout;
mod logger;
mod pagetable;
mod platform;
pub mod reexports;
#[cfg(platform = "qemu")]
mod semihosting;
mod services;
mod smccc;
mod stacks;

#[cfg(feature = "pauth")]
use crate::cpu_extensions::pauth;
use crate::{
    context::{CoresImpl, CpuDataIndex, initialise_contexts, update_contexts_suspend},
    gicv3::Gic,
    pagetable::{IdMap, OncePageTable, PageHeap},
    platform::Platform,
    services::{Services, psci::WakeUpReason},
};
#[cfg(not(test))]
pub use asm::bl31_warm_entrypoint;
#[cfg(all(target_arch = "aarch64", not(test)))]
use include_first::include_first;
use log::{debug, info};
use percore::Cores;
use spin::Once;

/// Handles early initialisation at the start of a cold boot, and then runs the main loop.
pub fn coldboot<
    const CORE_COUNT: usize,
    const PAGE_HEAP_PAGE_COUNT: usize,
    PlatformImpl: CpuDataIndex + Platform<IdMap = IdMap<PAGE_HEAP_PAGE_COUNT>>,
>(
    page_table: &OncePageTable<PAGE_HEAP_PAGE_COUNT>,
    page_heap: &'static PageHeap<PAGE_HEAP_PAGE_COUNT>,
    gic: &'static Once<Gic<'static, CORE_COUNT, PlatformImpl>>,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
) -> ! {
    PlatformImpl::init_with_early_mapping(arg0, arg1, arg2, arg3);

    page_table.init_runtime_mapping::<PlatformImpl>(page_heap);

    PlatformImpl::init(arg0, arg1, arg2, arg3);

    info!("Rust BL31 starting");
    debug!("Parameters: {arg0:#0x} {arg1:#0x} {arg2:#0x} {arg3:#0x}");

    debug!("Page table activated.");

    // SAFETY: This function never returns, so it is safe to enable PAuth part way through it.
    #[cfg(feature = "pauth")]
    unsafe {
        pauth::init::<PlatformImpl>();
    }

    // Set up GIC.
    gic.get().unwrap().init(&PlatformImpl::GIC_CONFIG);
    debug!("GIC configured.");

    let non_secure_entry_point = PlatformImpl::non_secure_entry_point();
    let secure_entry_point = PlatformImpl::secure_entry_point();
    #[cfg(feature = "rme")]
    let realm_entry_point = PlatformImpl::realm_entry_point();

    initialise_contexts::<PlatformImpl>(
        &non_secure_entry_point,
        &secure_entry_point,
        #[cfg(feature = "rme")]
        &realm_entry_point,
    );

    Services::get().run_loop()
}

#[cfg_attr(test, allow(unused))]
extern "C" fn psci_warmboot_entrypoint<PlatformImpl: CpuDataIndex + Platform>() -> ! {
    debug!(
        "Warmboot on core #{}",
        CoresImpl::<PlatformImpl>::core_index()
    );

    // SAFETY: This function never returns, so it is safe to enable PAuth part way through it.
    #[cfg(feature = "pauth")]
    unsafe {
        pauth::init::<PlatformImpl>();
    }

    let services = Services::get();

    match services.psci.handle_cpu_boot() {
        WakeUpReason::CpuOn(psci_entrypoint) => {
            // Power on for the first time or after CPU_OFF
            debug!("Wakeup from CPU_OFF");

            // TODO: Refactor handling of entrypoints to provide the warm boot entrypoints as well.
            // Also, at least some parts of the entrypoint should be provided by the service that
            // is responsible for a specific world (i.e. PC and args for SPMC come from the SPMD).
            let mut non_secure_entry_point = PlatformImpl::non_secure_entry_point();
            non_secure_entry_point.pc = psci_entrypoint.entry_point_address() as usize;
            non_secure_entry_point.args.fill(0);
            non_secure_entry_point.args[0] = psci_entrypoint.context_id();

            let mut secure_entry_point = PlatformImpl::secure_entry_point();
            secure_entry_point.pc = services.spmd.secondary_ep();
            secure_entry_point.args.fill(0);
            services.spmd.handle_wake_from_cpu_off();

            #[cfg(feature = "rme")]
            let realm_entry_point = PlatformImpl::realm_entry_point();

            initialise_contexts::<PlatformImpl>(
                &non_secure_entry_point,
                &secure_entry_point,
                #[cfg(feature = "rme")]
                &realm_entry_point,
            );
        }
        WakeUpReason::SuspendFinished(psci_entrypoint) => {
            debug!("Wakeup from CPU_SUSPEND");

            let secure_args = services.spmd.handle_wake_from_cpu_suspend();

            #[cfg(feature = "rme")]
            let realm_args = services.rmmd.handle_wake_from_cpu_suspend();

            // TODO: instead of modifying the context directly, should we rather pass the initial
            // gpregs of each world as arguments to run_loop()?
            update_contexts_suspend::<PlatformImpl>(
                psci_entrypoint,
                &secure_args,
                #[cfg(feature = "rme")]
                &realm_args,
            );
        }
    }

    services.run_loop()
}

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use super::*;
    use crate::{
        context::{CpuDataIndex, init_cpu_data_ptr},
        cpu::cpu_reset_handler,
        debug::{DEBUG, ENABLE_ASSERTIONS},
        pagetable::{PAGE_TABLE_ADDR, enable_mmu},
        stacks::set_my_stack,
    };
    use arm_sysregs::{Dit, SctlrEl3};
    use core::arch::global_asm;

    const DAIF_ABT_BIT: u32 = 1 << 2;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("zeromem.S"),
        include_str!("asm_macros_common_purge.S"),
        ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
        DEBUG = const DEBUG as i32,
        SCTLR_M_BIT = const SctlrEl3::M.bits(),
    );

    /// This CPU has been physically powered up. It is either resuming from suspend or has simply
    /// been turned on. In both cases, call the BL31 warmboot entrypoint.
    ///
    /// # Safety
    ///
    /// This must be called with the MMU turned off.
    #[unsafe(naked)]
    pub unsafe extern "C" fn bl31_warm_entrypoint<PlatformImpl: CpuDataIndex + Platform>() -> ! {
        naked_asm!(
            include_str!("asm_macros_common.S"),
            include_str!("bl31_warm_entrypoint.S"),
            include_str!("asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            SCTLR_M_BIT = const SctlrEl3::M.bits(),
            SCTLR_C_BIT = const SctlrEl3::C.bits(),
            SCTLR_WXN_BIT = const SctlrEl3::WXN.bits(),
            SCTLR_IESB_BIT = const SctlrEl3::IESB.bits(),
            SCTLR_A_BIT = const SctlrEl3::A.bits(),
            SCTLR_SA_BIT = const SctlrEl3::SA.bits(),
            SCTLR_I_BIT = const SctlrEl3::I.bits(),
            DAIF_ABT_BIT = const DAIF_ABT_BIT,
            DIT_BIT = const Dit::DIT.bits(),
            PAGE_TABLE_ADDR = sym PAGE_TABLE_ADDR,
            cpu_reset_handler = sym cpu_reset_handler,
            enable_mmu = sym enable_mmu::<PlatformImpl>,
            psci_warmboot_entrypoint = sym psci_warmboot_entrypoint::<PlatformImpl>,
            apply_reset_errata = sym errata_framework::apply_reset_errata,
            plat_set_my_stack = sym set_my_stack::<PlatformImpl>,
            init_cpu_data_ptr = sym init_cpu_data_ptr::<PlatformImpl>,
        );
    }

    /// This macro wraps a naked_asm block with `bti`, or any other universal
    /// prologue we'd still like added.
    ///
    /// Use this over `core::arch::naked_asm` by default, otherwise you may
    /// need to ensure that e.g. `bti` landing pads are in place yourself.
    macro_rules! naked_asm {
        ($($inner:tt)*) => {
           ::core::arch::naked_asm!("bti c", $($inner)*)
        }
    }
    pub(crate) use naked_asm;
}

/// Generates a naked function for the cold boot entrypoint assembly code.
#[cfg(all(target_arch = "aarch64", not(test)))]
#[include_first]
macro_rules! main_asm {
    ($platform:ty) => {
        type PlatformImplMain_ = $platform;

        mod main_asm {
            use super::PlatformImplMain_ as PlatformImpl;
            use $crate::platform::Platform;

            /// ABT bit for DAIFClr.
            ///
            /// Note that in DAIFClr the DAIF bits are in bits 0-3, rather than bits 6-9 as they are
            /// in the DAIF register itself.
            const DAIF_ABT_BIT: u32 = 1 << 2;

            /// The cold boot entrypoint, executed only by the primary cpu.
            #[unsafe(naked)]
            #[unsafe(no_mangle)]
            unsafe extern "C" fn bl31_entrypoint() -> ! {
                $crate::naked_asm!(
                    include_str!("asm_macros_common.S"),
                    include_str!("bl31_entrypoint.S"),
                    include_str!("asm_macros_common_purge.S"),
                    DEBUG = const $crate::debug::DEBUG as i32,
                    SCTLR_M_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::M.bits(),
                    SCTLR_C_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::C.bits(),
                    SCTLR_WXN_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::WXN.bits(),
                    SCTLR_IESB_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::IESB.bits(),
                    SCTLR_A_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::A.bits(),
                    SCTLR_SA_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::SA.bits(),
                    SCTLR_I_BIT = const $crate::reexports::arm_sysregs::SctlrEl3::I.bits(),
                    DAIF_ABT_BIT = const DAIF_ABT_BIT,
                    DIT_BIT = const $crate::reexports::arm_sysregs::Dit::DIT.bits(),
                    plat_cold_boot_handler = sym PlatformImpl::cold_boot_handler,
                    cpu_reset_handler = sym $crate::cpu::cpu_reset_handler,
                    init_early_page_tables = sym $crate::pagetable::early_pagetable::init_early_page_tables,
                    enable_mmu = sym $crate::pagetable::enable_mmu::<PlatformImpl>,
                    bl31_main = sym super::bl31_main,
                    apply_reset_errata = sym $crate::errata_framework::apply_reset_errata,
                    plat_set_my_stack = sym $crate::stacks::set_my_stack::<PlatformImpl>,
                    init_cpu_data_ptr = sym $crate::context::init_cpu_data_ptr::<PlatformImpl>,
                );
            }

        }
    };
}
#[allow(clippy::single_component_path_imports)]
#[cfg(all(target_arch = "aarch64", not(test)))]
pub(crate) use main_asm;

/// Generates `global_asm!` blocks for the given platform.
#[cfg(all(target_arch = "aarch64", not(test)))]
#[macro_export]
macro_rules! all_asm {
    ($platform:ty) => {
        $crate::context::context_asm!($platform);
        $crate::debug::debug_asm!($platform);
        $crate::stacks::stacks_asm!($platform);
        $crate::main_asm!($platform);
    };
}

#[cfg(all(target_arch = "aarch64", not(test)))]
pub(crate) use asm::naked_asm;

/// Generates static variables `LOGGER`, `GIC`, `PAGE_HEAP`, `PAGE_TABLE` and `PERCPU_DATA` for the
/// platform, and the `bl31_main` function.
#[macro_export]
macro_rules! statics {
    ($platform:ty) => {
        const _: () = assert!(
            size_of::<$crate::context::CpuData>()
                .is_multiple_of(align_of::<$crate::context::CpuData>())
        );
        const _: () = assert!(
            size_of::<$crate::context::CpuData>()
                .is_multiple_of(<$platform as $crate::platform::Platform>::CACHE_WRITEBACK_GRANULE)
        );
        const _: () = assert!(
            EARLY_PAGE_TABLE_SIZE
                <= (<$platform as $crate::platform::Platform>::CORE_COUNT - 1)
                    * $crate::stacks::STACK_SIZE,
            "The early page tables do not fit into the secondary core stack range."
        );

        type LogSinkImpl_ = <$platform as $crate::platform::Platform>::LogSinkImpl;

        static GIC: $crate::reexports::spin::Once<
            $crate::gicv3::Gic<
                { <$platform as $crate::platform::Platform>::CORE_COUNT },
                $platform,
            >,
        > = $crate::reexports::spin::Once::new();

        static LOGGER: $crate::logger::OnceLogger<LogSinkImpl_> = $crate::logger::OnceLogger::new();

        /// An array of pages which can be allocated for pagetables.
        pub static PAGE_HEAP: $crate::pagetable::PageHeap<
            { <$platform as $crate::platform::Platform>::PAGE_HEAP_PAGE_COUNT },
        > = $crate::pagetable::PageHeap::new();
        static PAGE_TABLE: $crate::pagetable::OncePageTable<
            { <$platform as $crate::platform::Platform>::PAGE_HEAP_PAGE_COUNT },
        > = $crate::pagetable::OncePageTable::new();

        #[cfg_attr(test, allow(dead_code))]
        static mut PERCPU_DATA: [$crate::context::CpuData;
            <$platform as $crate::platform::Platform>::CORE_COUNT] =
            [$crate::context::CpuData::EMPTY;
                <$platform as $crate::platform::Platform>::CORE_COUNT];

        #[cfg_attr(test, allow(unused))]
        extern "C" fn bl31_main(arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> ! {
            $crate::coldboot::<
                { <$platform as $crate::platform::Platform>::CORE_COUNT },
                { <$platform as $crate::platform::Platform>::PAGE_HEAP_PAGE_COUNT },
                $platform,
            >(&PAGE_TABLE, &PAGE_HEAP, &GIC, arg0, arg1, arg2, arg3)
        }
    };
}

/// Generates a panic handler which will log the panic message to `LOGGER` then loop forever.
#[macro_export]
macro_rules! panic_handler {
    () => {
        #[cfg(not(test))]
        #[panic_handler]
        fn panic(info: &core::panic::PanicInfo) -> ! {
            use $crate::logger::LogSink;

            if let Some(sink) = LOGGER.log_sink() {
                writeln!(sink, "{info}");
            }
            loop {}
        }
    };
}
