// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Errata for the DynamIQ Shared Unit.

use crate::{
    errata_framework::{Cve, Erratum, ErratumId, ErratumType},
    naked_asm,
};

/// DynamIQ Shared Unit erratum 3396010.
///
/// This is relevant for both C1 Ultra and C1 Pro, so just checks the DSU version rather than the
/// CPU MIDR.
#[allow(unused)]
pub struct DsuErratum3396010;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x2.
unsafe impl Erratum for DsuErratum3396010 {
    const ID: ErratumId = 3_396_010;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        // Check if DSU version is r0p0.
        naked_asm!(
            "mov	x0, #0",
            // s3_0_c15_c3_1 is clusteridr_el1.
            "mrs	x1, s3_0_c15_c3_1",
            // DSU variant and revision bitfields in clusteridr are adjacent.
            "ubfx	x1, x1, #{CLUSTERIDR_REV_SHIFT}, #({CLUSTERIDR_REV_BITS} + {CLUSTERIDR_VAR_BITS})",
            "mov	x2, #(0x0 << {CLUSTERIDR_REV_SHIFT})",
            "cmp	x1, x2",
            "b.hi	1f",
            "mov	x0, #1",
            "1:",
            "ret",
            CLUSTERIDR_REV_SHIFT = const 0,
            CLUSTERIDR_REV_BITS = const 4,
            CLUSTERIDR_VAR_BITS = const 4,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // If erratum applies, disable high-level clock gating.
        naked_asm!(
            // s3_0_c15_c3_3 is clusteractlr_el1.
            "mrs	x0, s3_0_c15_c3_3",
            "orr	x0, x0, #{CLUSTERACTLR_EL1_DISABLE_SCLK_GATING}",
            "msr	s3_0_c15_c3_3, x0",
            "ret",
            CLUSTERACTLR_EL1_DISABLE_SCLK_GATING = const 3u64 << 15,
        );
    }
}
