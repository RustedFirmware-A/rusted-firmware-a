// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Pointer Authentication extension (FEAT_PAuth)
//!
//! This module provides support for enabling PAuth at EL3. When RF-A is compiled with PAuth
//! enabled, PAC instructions are inserted into function preludes but the signing and authentication
//! operations are no-ops until PAuth is enabled at runtime. Therefore, it is desirable to enable
//! PAuth as early as possible and care needs to be taken not to enable PAuth part way through a
//! function that will return, because the signing will be a no-op and then attempting to
//! authenticate will fail. For these reasons, PAuth is handled outside the standard CPU extensions
//! framework.

use crate::{
    aarch64::isb,
    context::cpu_data_set_apkey,
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{
    ApiakeyhiEl1, ApiakeyloEl1, Sctlr2El3, SctlrEl3, read_id_aa64isar1_el1, read_id_aa64isar2_el1,
    read_sctlr_el3, read_sctlr2_el3, write_apiakeyhi_el1, write_apiakeylo_el1, write_sctlr_el3,
    write_sctlr2_el3,
};

const PAUTH_LR_IMPLEMENTED: u8 = 0b110;

/// Indicates whether FEAT_PAuth_LR is implemented.
fn is_feat_pauth_lr_present() -> bool {
    let id_aa64isar1_el1 = read_id_aa64isar1_el1();
    // FEAT_PAuth_LR support is indicated by up to 3 fields, where if one or more of these is 0b0110
    // then the feature is present.
    //   1) id_aa64isr1_el1.api
    //   2) id_aa64isr1_el1.apa
    //   3) id_aa64isr2_el1.apa3
    id_aa64isar1_el1.apa() == PAUTH_LR_IMPLEMENTED
        || id_aa64isar1_el1.api() == PAUTH_LR_IMPLEMENTED
        || read_id_aa64isar2_el1().apa3() == PAUTH_LR_IMPLEMENTED
}

/// Setup the PAuth registers and the CPU data with the PAuth key.
fn set_apkey() {
    let key = PlatformImpl::init_apkey();

    // SAFETY: We haven't yet enabled PAuth, so it is safe to set the key.
    unsafe {
        write_apiakeylo_el1(ApiakeyloEl1::from_bits_retain(key as u64));
        write_apiakeyhi_el1(ApiakeyhiEl1::from_bits_retain((key >> 64) as u64));
    }

    exception_free(|token| cpu_data_set_apkey(token, key));
}

/// Enables Pointer Authentication at EL3.
///
/// # Safety
///
/// The caller must only call this function from either a function with no PAuth guards or one that
/// never returns, otherwise authentication will fail when the caller's function returns. This
/// function is always inlined to ensure that it does not introduce PAuth guards of its own.
#[inline(always)]
pub unsafe fn init() {
    set_apkey();

    // SAFETY: It is safe to enable pointer authentication here because this function is always
    // inlined so it does not have PAuth guards and the caller has called it from a context without
    // PAuth guards.
    unsafe {
        write_sctlr_el3(read_sctlr_el3() | SctlrEl3::ENIA);
    }

    if is_feat_pauth_lr_present() {
        // SAFETY: Enabling PAuth_LR is safe here for the same reasons as PAuth.
        unsafe {
            write_sctlr2_el3(read_sctlr2_el3() | Sctlr2El3::ENPACM);
        }
    }

    isb();
}
