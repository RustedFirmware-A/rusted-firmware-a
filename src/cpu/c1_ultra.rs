// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{aarch64::isb, cpu::Cpu, naked_asm};
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
