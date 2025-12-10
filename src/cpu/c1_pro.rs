// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    errata_framework::{
        Cve, Erratum, ErratumId, ErratumType, RevisionVariant, implement_erratum_check,
    },
    naked_asm,
};
use arm_sysregs::MidrEl1;

#[allow(unused)]
const MIDR: MidrEl1 = MidrEl1::from_bits_retain(0x410F_D8B0);

#[allow(unused)]
pub struct Erratum3619847;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3619847 {
    const ID: ErratumId = 3_619_847;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(MIDR, RevisionVariant::new(0, 0), RevisionVariant::new(1, 0));
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 42 in C1_PRO_IMP_CPUACTLR2_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_1",
            "orr x1, x1, #(1 << 42)",
            "msr s3_0_c15_c1_1, x1",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum3694158;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3694158 {
    const ID: ErratumId = 3_694_158;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(MIDR, RevisionVariant::new(0, 0), RevisionVariant::new(1, 2));
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        naked_asm!(
            "mov x0, #5",
            "msr s3_6_c15_c8_0, x0",
            "isb",
            "ldr x0, =0xd503329f",
            "msr s3_6_c15_c8_2, x0",
            "ldr x0, =0xfffff3ff",
            "msr s3_6_c15_c8_3, x0",
            "mov x0, #(1 << 0 | 3 << 4 | 0xf << 6)",
            "orr x0, x0, #1 << 22",
            "orr x0, x0, #1 << 32",
            "msr s3_6_c15_c8_1, x0",
            "isb",
            "ret",
        )
    }
}
