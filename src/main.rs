// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RF-A: A new implementation of TF-A for AArch64.

#![cfg_attr(not(test), no_main)]
#![cfg_attr(not(test), no_std)]

mod aarch64;
mod context;
mod cpu;
#[cfg(not(test))]
mod crash_console;
mod debug;
mod dram;
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
#[cfg(not(test))]
mod stacks;
mod sysregs;

use crate::{
    context::{CoresImpl, initialise_contexts, update_contexts_suspend},
    platform::{Platform, PlatformImpl},
    services::{Services, psci::WakeUpReason},
};
use log::{debug, info};
use percore::Cores;

#[unsafe(no_mangle)]
extern "C" fn bl31_main(bl31_params: u64, platform_params: u64) -> ! {
    PlatformImpl::init_before_mmu();
    info!("Rust BL31 starting");
    info!("Parameters: {bl31_params:#0x} {platform_params:#0x}");

    // Set up page table.
    pagetable::init();
    info!("Page table activated.");

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

#[unsafe(no_mangle)]
extern "C" fn psci_warmboot_entrypoint() -> ! {
    pagetable::enable();
    debug!("Warmboot on core #{}", CoresImpl::core_index());

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

            // TODO: instead of modifying the context directly, should we rather pass the initial
            // gpregs of each world as arguments to run_loop()?
            update_contexts_suspend(psci_entrypoint, &secure_args);
        }
    }

    services.run_loop()
}

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use super::*;
    use crate::debug::{DEBUG, ENABLE_ASSERTIONS};
    use arm_sysregs::SctlrEl3;
    use core::arch::global_asm;

    const MDCR_MTPME_BIT: u64 = 1 << 28;
    const MDCR_TPM_BIT: u64 = 1 << 6;
    const MDCR_TDA_BIT: u64 = 1 << 9;
    const MDCR_TDOSA_BIT: u64 = 1 << 10;
    const MDCR_SDD_BIT: u64 = 1 << 16;
    const MDCR_SPME_BIT: u64 = 1 << 17;
    const MDCR_TTRF_BIT: u64 = 1 << 19;
    const MDCR_SCCD_BIT: u64 = 1 << 23;
    const MDCR_NSTBE_BIT: u64 = 1 << 26;
    const MDCR_MCCD_BIT: u64 = 1 << 34;
    const MDCR_SPD32_MDCR_SPD32_DISABLE: u64 = 0x2 << 14;
    const MDCR_NSTB_MDCR_NSTB_EL1: u64 = 0x3 << 24;

    const PMCR_EL0_RESET_VAL: u32 = 0;
    const PMCR_EL0_D_BIT: u32 = 1 << 3;
    const PMCR_EL0_X_BIT: u32 = 1 << 4;
    const PMCR_EL0_DP_BIT: u32 = 1 << 5;
    const PMCR_EL0_LC_BIT: u32 = 1 << 6;
    const PMCR_EL0_LP_BIT: u32 = 1 << 7;

    const DAIF_ABT_BIT: u32 = 1 << 2;

    const CPTR_EL3_RESET_VAL: u32 =
        (TCPAC_BIT | TAM_BIT | TTA_BIT | TFP_BIT) & !(CPTR_EZ_BIT | ESM_BIT);
    const CPTR_EZ_BIT: u32 = 1 << 8;
    const TFP_BIT: u32 = 1 << 10;
    const ESM_BIT: u32 = 1 << 12;
    const TTA_BIT: u32 = 1 << 20;
    const TAM_BIT: u32 = 1 << 30;
    const TCPAC_BIT: u32 = 1 << 31;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("misc_helpers.S"),
        include_str!("bl31_entrypoint.S"),
        include_str!("asm_macros_common_purge.S"),
        ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
        DEBUG = const DEBUG as i32,
        SCTLR_M_BIT = const SctlrEl3::M.bits(),
        SCTLR_A_BIT = const SctlrEl3::A.bits(),
        SCTLR_SA_BIT = const SctlrEl3::SA.bits(),
        SCTLR_I_BIT = const SctlrEl3::I.bits(),
        MDCR_EL3_RESET_VAL = const MDCR_MTPME_BIT,
        MDCR_TPM_BIT = const MDCR_TPM_BIT,
        MDCR_TDA_BIT = const MDCR_TDA_BIT,
        MDCR_TDOSA_BIT = const MDCR_TDOSA_BIT,
        MDCR_SDD_BIT = const MDCR_SDD_BIT,
        MDCR_SPME_BIT = const MDCR_SPME_BIT,
        MDCR_TTRF_BIT = const MDCR_TTRF_BIT,
        MDCR_SCCD_BIT = const MDCR_SCCD_BIT,
        MDCR_NSTBE_BIT = const MDCR_NSTBE_BIT,
        MDCR_MCCD_BIT = const MDCR_MCCD_BIT,
        MDCR_SPD32_MDCR_SPD32_DISABLE = const MDCR_SPD32_MDCR_SPD32_DISABLE,
        MDCR_NSTB_MDCR_NSTB_EL1 = const MDCR_NSTB_MDCR_NSTB_EL1,
        ID_AA64DFR0_TRACEFILT_SHIFT = const 40,
        ID_AA64DFR0_TRACEFILT_LENGTH = const 4,
        ID_AA64DFR0_TRACEVER_SHIFT = const 4,
        ID_AA64DFR0_TRACEVER_LENGTH = const 4,
        PMCR_EL0_RESET_VAL = const PMCR_EL0_RESET_VAL,
        PMCR_EL0_D_BIT = const PMCR_EL0_D_BIT,
        PMCR_EL0_X_BIT = const PMCR_EL0_X_BIT,
        PMCR_EL0_DP_BIT = const PMCR_EL0_DP_BIT,
        PMCR_EL0_LC_BIT = const PMCR_EL0_LC_BIT,
        PMCR_EL0_LP_BIT = const PMCR_EL0_LP_BIT,
        DAIF_ABT_BIT = const DAIF_ABT_BIT,
        CPTR_EL3_RESET_VAL = const CPTR_EL3_RESET_VAL,
        TCPAC_BIT = const TCPAC_BIT,
        TTA_BIT = const TTA_BIT,
        TFP_BIT = const TFP_BIT,
        plat_cold_boot_handler = sym PlatformImpl::cold_boot_handler,
    );

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
