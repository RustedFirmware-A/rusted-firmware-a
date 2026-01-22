// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SIMD, SVE and SME support.

#[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
mod simd_sel1;

use super::CpuExtension;
use crate::{
    aarch64::isb,
    context::{PerWorldContext, World},
};
use arm_sysregs::{
    CptrEl3, IdAa64smfr0El1, ScrEl3, SmcrEl3, ZcrEl3, read_cptr_el3, read_id_aa64pfr0_el1,
    read_id_aa64pfr1_el1, read_id_aa64smfr0_el1, write_cptr_el3, write_smcr_el3, write_zcr_el3,
};

const FP_NOT_SUPPORTED: u8 = 0xf;
const ADVSIMD_NOT_SUPPORTED: u8 = 0xf;

/// Returns whether SVE and SME access must be permitted based on given `world`.
fn needs_sve_sme(world: World) -> bool {
    match world {
        World::NonSecure => true,
        World::Secure if cfg!(feature = "sel2") => true,

        #[cfg(feature = "rme")]
        World::Realm => true,

        _ => false,
    }
}
/// FEAT_SVE support.
///
/// Enables NS world SVE register access and configures the maximum SVE vector length.
struct Sve {
    /// Limits the Effective Non-streaming SVE vector length to `vector_length` bits.
    vector_length: u64,
}

impl Sve {
    #[allow(unused)]
    const fn new(vector_length: u64) -> Self {
        assert!(
            vector_length.is_multiple_of(128) && vector_length >= 128 && vector_length <= 2048,
            "Invalid SVE vector length"
        );
        Self { vector_length }
    }

    fn is_present() -> bool {
        read_id_aa64pfr0_el1().is_feat_sve_present()
    }

    fn init(&self) {
        // Temporarily allow SVE register access, to configure the maximum SVE vector length.
        let cptr_el3 = read_cptr_el3();
        // SAFETY: We only allowed SVE instructions.
        unsafe {
            write_cptr_el3(cptr_el3 | CptrEl3::EZ);
        }
        isb();

        // ZCR_EL3[3:0]:
        // Requests an Effective Non-streaming SVE vector length at EL3 of (LEN+1)*128 bits.
        // SAFETY: We don't use any SVE instructions, so this doesn't affect us.
        unsafe {
            write_zcr_el3(ZcrEl3::from_bits_retain(self.vector_length / 128 - 1));
        }

        // Restore CPTR_EL3.
        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_cptr_el3(cptr_el3);
        }
    }

    fn configure_per_world(world: World, ctx: &mut PerWorldContext) {
        // Allow SVE register access to normal world unconditionally,
        // secure world if S-EL2 enabled, and realm world if enabled.
        if needs_sve_sme(world) {
            ctx.cptr_el3 |= CptrEl3::EZ;
        }
    }
}

/// FEAT_SME support.
///
/// Enables NS world SME register access and configures the maximum Streaming SVE (SSVE) vector
/// length.
struct Sme {
    /// Limits the Effective Streaming SVE vector length to `vector_length` bits.
    vector_length: u64,
}

impl Sme {
    const fn new(vector_length: u64) -> Self {
        assert!(
            vector_length.is_multiple_of(128) && vector_length >= 128 && vector_length <= 2048,
            "Invalid SSVE vector length"
        );
        Self { vector_length }
    }

    fn is_present() -> bool {
        read_id_aa64pfr1_el1().is_feat_sme_present()
    }

    fn init(&self) {
        // Temporarily allow SME register access, to configure the maximum SSVE vector length.
        let cptr_el3 = read_cptr_el3();
        // SAFETY: We only allowed SME instructions.
        unsafe {
            write_cptr_el3(cptr_el3 | CptrEl3::ESM);
        }
        isb();

        // Configure maximum SSVE vector length.
        let mut smcr_el3 = SmcrEl3::from_ssve_vector_len(self.vector_length);

        if read_id_aa64smfr0_el1().contains(IdAa64smfr0El1::FA64) {
            smcr_el3 |= SmcrEl3::FA64;
        }

        // Enable access to ZT0 registers if SME2 is present.
        if read_id_aa64pfr1_el1().is_feat_sme2_present() {
            smcr_el3 |= SmcrEl3::EZT0;
        }

        // Configure SMCR_EL3 for all worlds.
        // SAFETY: We don't use any SME instructions, so this doesn't affect us.
        unsafe {
            write_smcr_el3(smcr_el3);
        }

        // Restore CPTR_EL3.
        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_cptr_el3(cptr_el3);
        }
    }

    fn configure_per_world(world: World, ctx: &mut PerWorldContext) {
        // Allow SME register access to normal world unconditionally,
        // secure world if S-EL2 enabled, and realm world if enabled.
        if needs_sve_sme(world) {
            ctx.cptr_el3 |= CptrEl3::ESM;
            ctx.scr_el3 |= ScrEl3::ENTP2;
        }
    }
}

/// Enables FP, SIMD, SVE and SME CPU extensions.
pub struct Simd {
    sve: Option<Sve>,
    sme: Option<Sme>,
}

impl Simd {
    /// Creates a new `Simd` extension with SVE and SME disabled.
    #[allow(unused)]
    #[allow(clippy::self_named_constructors)]
    pub const fn simd() -> Self {
        Self {
            sve: None,
            sme: None,
        }
    }

    /// Creates a new `Simd` extension.
    ///
    /// Enables SVE. Configures the maximum vector length for SVE to `vector_length`.
    ///
    /// If `enable_sme` is set, SME extension is enabled as well and SSVE vector length is also set
    /// to `vector_length`.
    #[allow(unused)]
    pub const fn sve(vector_length: u64, enable_sme: bool) -> Self {
        Self {
            sve: Some(Sve::new(vector_length)),
            sme: if enable_sme {
                Some(Sme::new(vector_length))
            } else {
                None
            },
        }
    }
}

impl CpuExtension for Simd {
    fn is_present(&self) -> bool {
        // We assume that SVE or SME presence implies SIMD presence,
        // so its sufficient to only check for the 'base' extension.
        let id_aa64pfr0_el1 = read_id_aa64pfr0_el1();
        id_aa64pfr0_el1.fp() != FP_NOT_SUPPORTED
            && id_aa64pfr0_el1.advsimd() != ADVSIMD_NOT_SUPPORTED
    }

    fn init(&self) {
        if let Some(sve) = &self.sve
            && Sve::is_present()
        {
            sve.init();
        }
        if let Some(sme) = &self.sme
            && Sme::is_present()
        {
            sme.init();
        }
    }

    fn configure_per_world(&self, world: World, ctx: &mut PerWorldContext) {
        // Allow FP/SIMD register accesses in every World.
        ctx.cptr_el3 -= CptrEl3::TFP;

        if self.sve.is_some() && Sve::is_present() {
            Sve::configure_per_world(world, ctx);
        }
        if self.sme.is_some() && Sme::is_present() {
            Sme::configure_per_world(world, ctx);
        }
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn save_context(&self, world: World) {
        use crate::platform::exception_free;

        let has_sve = self.sve.is_some() && Sve::is_present();
        let has_sme = self.sme.is_some() && Sme::is_present();

        // Temporarily allow access to save context
        let cptr_el3 = read_cptr_el3();
        // SAFETY: We only allowed SVE and SME instructions.
        unsafe {
            write_cptr_el3((cptr_el3 - CptrEl3::TFP) | CptrEl3::EZ | CptrEl3::ESM);
        }
        isb();

        if world == World::NonSecure && has_sve {
            exception_free(|token| {
                simd_sel1::NS_SVE_CTX.get().borrow_mut(token).save(has_sme);
            })
        } else {
            exception_free(|token| {
                simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].save();
            })
        }

        // Restore Architectural Feature Trap Register.
        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_cptr_el3(cptr_el3);
        }
        isb();
    }

    #[cfg(all(target_arch = "aarch64", not(feature = "sel2")))]
    fn restore_context(&self, world: World) {
        use crate::platform::exception_free;

        let has_sve = self.sve.is_some() && Sve::is_present();
        let has_sme = self.sme.is_some() && Sme::is_present();

        // Temporarily allow access to restore context
        let cptr_el3 = read_cptr_el3();
        // SAFETY: We only allowed SVE and SME instructions.
        unsafe {
            write_cptr_el3((cptr_el3 - CptrEl3::TFP) | CptrEl3::EZ | CptrEl3::ESM);
        }
        isb();

        if world == World::NonSecure && has_sve {
            exception_free(|token| {
                simd_sel1::NS_SVE_CTX
                    .get()
                    .borrow_mut(token)
                    .restore(has_sme);
            })
        } else {
            exception_free(|token| {
                simd_sel1::SIMD_CTX.get().borrow_mut(token)[world].restore();
            })
        }

        // Restore Architectural Feature Trap Register.
        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_cptr_el3(cptr_el3);
        }
        isb();
    }
}
