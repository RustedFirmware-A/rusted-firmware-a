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

read_write_sysreg!(imp_cpupwrctlr_el1: s3_0_c15_c2_7, u64, safe_read, safe_write);
const IMP_CPUPWRCTLR_EL1_CORE_PWRDN_EN_BIT: u64 = 0x1;

pub struct C1Ultra;

#[allow(unused)]
/// SAFETY: `reset_handler` and `dump_registers` are implemented as naked functions and only clobber
/// x1.
unsafe impl Cpu for C1Ultra {
    const MIDR: MidrEl1 = MidrEl1::from_bits_retain(0x410F_D8C0);

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
        static C1_ULTRA_REGS: [u8; 14] = *b"cpuectlr_el1\0\0";

        naked_asm!(
            "adr x6, {c1_ultra_regs}",
            "mrs x8, s3_0_c15_c1_4",
            "ret",
            c1_ultra_regs = sym C1_ULTRA_REGS,
        );
    }

    fn power_down_level0() {
        write_imp_cpupwrctlr_el1(read_imp_cpupwrctlr_el1() | IMP_CPUPWRCTLR_EL1_CORE_PWRDN_EN_BIT);
        isb();
    }

    fn power_down_level1() {
        Self::power_down_level0();
    }
}

#[allow(unused)]
pub struct Erratum3658374;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3658374 {
    const ID: ErratumId = 3_658_374;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Runtime;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        naked_asm!("ret")
    }
}

#[allow(unused)]
pub struct Erratum3705939;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3705939 {
    const ID: ErratumId = 3_705_939;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 48 in C1_ULTRA_IMP_CPUACTLR_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_0",
            "orr x1, x1, #(1 << 48)",
            "msr s3_0_c15_c1_0, x1",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum3815514;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3815514 {
    const ID: ErratumId = 3_815_514;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 13 in C1_ULTRA_IMP_CPUACTLR5_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c8_0",
            "orr x1, x1, #(1 << 13)",
            "msr s3_0_c15_c8_0, x1",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum3865171;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3865171 {
    const ID: ErratumId = 3_865_171;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 22 in C1_ULTRA_IMP_CPUACTLR2_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_1",
            "orr x1, x1, #(1 << 22)",
            "msr s3_0_c15_c1_1, x1",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum3926381;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum3926381 {
    const ID: ErratumId = 3_926_381;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(1, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        naked_asm!(
            // Convert WFx to NOP.
            "ldr x0,=0x0",
            // C1_ULTRA_IMP_CPUPSELR_EL3
            "msr s3_6_c15_c8_0, x0",
            "ldr x0,=0xD503205f",
            // C1_ULTRA_IMP_CPUPOR_EL3
            "msr s3_6_c15_c8_2, x0",
            "ldr x0,=0xFFFFFFDF",
            // C1_ULTRA_IMP_CPUPMR_EL3
            "msr s3_6_c15_c8_3, x0",
            "ldr x0,=0x1000002043ff",
            // C1_ULTRA_IMP_CPUPCR_EL3
            "msr s3_6_c15_c8_1, x0",
            // Convert WFxT to NOP.
            "ldr x0,=0x1",
            // C1_ULTRA_IMP_CPUPSELR_EL3
            "msr s3_6_c15_c8_0, x0",
            "ldr x0,=0xD5031000",
            // C1_ULTRA_IMP_CPUPOR_EL3
            "msr s3_6_c15_c8_2, x0",
            "ldr x0,=0xFFFFFFC0",
            // C1_ULTRA_IMP_CPUPMR_EL3
            "msr s3_6_c15_c8_3, x0",
            "ldr x0,=0x1000002043ff",
            // C1_ULTRA_IMP_CPUPCR_EL3
            "msr s3_6_c15_c8_1, x0",
            "isb",
            "ret",
        )
    }
}

#[allow(unused)]
pub struct Erratum4102704;

// SAFETY: `check` and `workaround` are both implemented using naked_asm, don't use the stack or
// memory, and only clobber x0-x4.
unsafe impl Erratum for Erratum4102704 {
    const ID: ErratumId = 4_102_704;
    const CVE: Cve = 0;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    #[unsafe(naked)]
    extern "C" fn check() -> bool {
        implement_erratum_check!(
            C1Ultra::MIDR,
            RevisionVariant::new(0, 0),
            RevisionVariant::NOT_FIXED,
        );
    }

    #[unsafe(naked)]
    extern "C" fn workaround() {
        // Set bit 23 in C1_ULTRA_IMP_CPUACTLR4_EL1.
        naked_asm!(
            "mrs x1, s3_0_c15_c1_3",
            "orr x1, x1, #(1 << 23)",
            "msr s3_0_c15_c1_3, x1",
            "ret",
        )
    }
}
