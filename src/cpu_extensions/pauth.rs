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

use crate::{aarch64::isb, context::set_percore_pauth_apiakey, platform::Platform};
use arm_sysregs::{
    el1::{
        accessors::{write_apiakeyhi_el1, write_apiakeylo_el1},
        helpers::is_feat_pauth_lr_present,
        registers::{ApiakeyhiEl1, ApiakeyloEl1},
    },
    el3::{
        accessors::{read_sctlr_el3, read_sctlr2_el3, write_sctlr_el3, write_sctlr2_el3},
        registers::{Sctlr2El3, SctlrEl3},
    },
};

/// Setup the PAuth registers and the CPU data with the PAuth key.
fn set_apkey<PlatformImpl: Platform>() {
    let key = PlatformImpl::init_apkey();

    // SAFETY: We haven't yet enabled PAuth, so it is safe to set the key.
    unsafe {
        write_apiakeylo_el1(ApiakeyloEl1::from_bits_retain(key as u64));
        write_apiakeyhi_el1(ApiakeyhiEl1::from_bits_retain((key >> 64) as u64));
    }

    set_percore_pauth_apiakey(key);
}

/// Enables Pointer Authentication at EL3.
///
/// # Safety
///
/// The caller must only call this function from either a function with no PAuth guards or one that
/// never returns, otherwise authentication will fail when the caller's function returns. This
/// function is always inlined to ensure that it does not introduce PAuth guards of its own.
#[inline(always)]
pub unsafe fn init<PlatformImpl: Platform>() {
    set_apkey::<PlatformImpl>();

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
