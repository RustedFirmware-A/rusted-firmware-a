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
#[cfg_attr(test, path = "layout_fake.rs")]
mod layout;
mod logger;
mod pagetable;
mod platform;
#[cfg(platform = "qemu")]
mod semihosting;
mod services;
mod smccc;
mod stacks;

#[cfg(feature = "pauth")]
use crate::cpu_extensions::pauth;
use crate::{
    context::{CoresImpl, initialise_contexts, update_contexts_suspend},
    platform::{Platform, PlatformImpl},
    services::{Services, psci::WakeUpReason},
};
#[cfg(not(test))]
pub use asm::bl31_warm_entrypoint;
use log::{debug, info};
use percore::Cores;

#[cfg_attr(test, allow(unused))]
extern "C" fn bl31_main(arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> ! {
    PlatformImpl::init_with_early_mapping(arg0, arg1, arg2, arg3);

    pagetable::init_runtime_mapping();

    PlatformImpl::init(arg0, arg1, arg2, arg3);

    info!("Rust BL31 starting");
    info!("Parameters: {arg0:#0x} {arg1:#0x} {arg2:#0x} {arg3:#0x}");

    info!("Page table activated.");

    // SAFETY: This function never returns, so it is safe to enable PAuth part way through it.
    #[cfg(feature = "pauth")]
    unsafe {
        pauth::init();
    }

    // Set up GIC.
    gicv3::init();
    info!("GIC configured.");

    let non_secure_entry_point = PlatformImpl::non_secure_entry_point();
    let secure_entry_point = PlatformImpl::secure_entry_point();
    #[cfg(feature = "rme")]
    let realm_entry_point = PlatformImpl::realm_entry_point();

    initialise_contexts(
        &non_secure_entry_point,
        &secure_entry_point,
        #[cfg(feature = "rme")]
        &realm_entry_point,
    );

    Services::get().run_loop();
}

#[cfg_attr(test, allow(unused))]
extern "C" fn psci_warmboot_entrypoint() -> ! {
    debug!("Warmboot on core #{}", CoresImpl::core_index());

    // SAFETY: This function never returns, so it is safe to enable PAuth part way through it.
    #[cfg(feature = "pauth")]
    unsafe {
        pauth::init();
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

            initialise_contexts(
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
            update_contexts_suspend(
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
        cpu::cpu_reset_handler,
        debug::{DEBUG, ENABLE_ASSERTIONS},
        pagetable::{PAGE_TABLE_ADDR, early_pagetable::init_early_page_tables, enable_mmu},
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

    /// The cold boot entrypoint, executed only by the primary cpu.
    #[unsafe(naked)]
    #[unsafe(no_mangle)]
    unsafe extern "C" fn bl31_entrypoint() -> ! {
        naked_asm!(
            include_str!("asm_macros_common.S"),
            include_str!("bl31_entrypoint.S"),
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
            plat_cold_boot_handler = sym PlatformImpl::cold_boot_handler,
            cpu_reset_handler = sym cpu_reset_handler,
            init_early_page_tables = sym init_early_page_tables,
            enable_mmu = sym enable_mmu,
            bl31_main = sym bl31_main,
            apply_reset_errata = sym errata_framework::apply_reset_errata,
            plat_set_my_stack = sym set_my_stack,
        );
    }

    /// This CPU has been physically powered up. It is either resuming from suspend or has simply
    /// been turned on. In both cases, call the BL31 warmboot entrypoint.
    ///
    /// # Safety
    ///
    /// This must be called with the MMU turned off.
    #[unsafe(naked)]
    pub unsafe extern "C" fn bl31_warm_entrypoint() -> ! {
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
            enable_mmu = sym enable_mmu,
            psci_warmboot_entrypoint = sym psci_warmboot_entrypoint,
            apply_reset_errata = sym errata_framework::apply_reset_errata,
            plat_set_my_stack = sym set_my_stack,
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

#[cfg(all(target_arch = "aarch64", not(test)))]
pub(crate) use asm::naked_asm;
