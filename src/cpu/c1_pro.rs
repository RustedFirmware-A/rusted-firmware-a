// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    aarch64::isb,
    cpu::Cpu,
    errata_framework::{
        Cve, Erratum, ErratumId, ErratumType, RevisionVariant, implement_erratum_check,
    },
    naked_asm,
};
use arm_sysregs::{MidrEl1, read_write_sysreg};

pub struct C1Pro;

#[allow(unused)]
/// SAFETY: `reset_handler` and `dump_registers` are implemented as naked functions and only clobber
/// x1.
unsafe impl Cpu for C1Pro {
    const MIDR: MidrEl1 = MidrEl1::from_bits_retain(0x410F_D8B0);

    #[unsafe(naked)]
    extern "C" fn reset_handler() {
        naked_asm!(
            // Disable speculative loads by zeroing SSBS.
            "msr s3_3_c4_c2_6, xzr",
            // Clear bit 0 (CORE_PWRDN_EN) in IMP_CPUPWRCTLR_EL1, to work around a model bug where
            // it isn't cleared on reset.
            "mrs x1, s3_0_c15_c2_7",
            "bic x1, x1, #(1 << 0)",
            "msr s3_0_c15_c2_7, x1",
            "ret"
        );
    }

    #[unsafe(naked)]
    extern "C" fn dump_registers() {
        static C1_PRO_REGS: [u8; 18] = *b"imp_cpuectlr_el1\0\0";

        naked_asm!(
            "adr x6, {c1_pro_regs}",
            "mrs x8, s3_0_c15_c1_4",
            "ret",
            c1_pro_regs = sym C1_PRO_REGS,
        );
    }

    fn power_down_level0() {
        let cpupwrctlr = read_cpupwrctlr();
        write_cpupwrctlr(cpupwrctlr | CORE_PWRDN_ENABLE_BIT_MASK);
        isb();

        if Erratum3686597::check() {
            Erratum3686597::workaround();
        }
    }

    fn power_down_level1() {
        Self::power_down_level0();
    }
}

#[allow(unused)]
pub struct Erratum3300099;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3300099 {
    const ID: ErratumId = 3_300_099;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Runtime;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 1),
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        naked_asm!("ret")
    }
}

#[allow(unused)]
pub struct Erratum3773617;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3773617 {
    const ID: ErratumId = 3_773_617;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Runtime;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(1, 1),
            RevisionVariant::new(1, 2),
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        naked_asm!("ret")
    }
}

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
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 0)
        );
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

/// Workaround for CME-related powerdown transition deadlocks.
pub struct Erratum3686597;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3686597 {
    const ID: ErratumId = 3_686_597;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Runtime;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 1),
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 57 in C1_PRO_IMP_CPUECTLR_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_4",
            "orr x1, x1, #(1 << 57)",
            "msr s3_0_c15_c1_4, x1",
            "dsb sy",
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
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 2)
        );
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

#[allow(unused)]
pub struct Erratum3684268;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3684268 {
    const ID: ErratumId = 3_684_268;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 1)
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 49 in C1_PRO_IMP_CPUECTLR2_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_5",
            "orr x1, x1, #(1 << 49)",
            "msr s3_0_c15_c1_5, x1",
            "dsb sy",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum3706576;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3706576 {
    const ID: ErratumId = 3_706_576;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Pro::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::new(1, 1)
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 37 in C1_PRO_IMP_CPUACTLR2_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_1",
            "orr x1, x1, #(1 << 37)",
            "msr s3_0_c15_c1_1, x1",
            "ret",
        )
    }
}

read_write_sysreg!(cpupwrctlr: s3_0_c15_c2_7, u64, safe_read, safe_write);
const CORE_PWRDN_ENABLE_BIT_MASK: u64 = 0x1;
