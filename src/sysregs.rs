// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#![allow(unused)]

#[cfg(test)]
pub mod fake;

use arm_sysregs::{read_sysreg, read_write_sysreg, write_sysreg};
use bitflags::bitflags;
use core::fmt::{self, Debug, Formatter};

/// Constants for PMCR_EL0 fields.
pub mod pmcr {
    /// Disable cycle counter when event counting is prohibited.
    pub const DP: u64 = 1 << 5;
}

read_sysreg!(id_aa64mmfr1_el1, u64, safe, fake::SYSREGS);
read_write_sysreg!(actlr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(actlr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr0_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr0_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr1_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr1_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(amair_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(amair_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cntfrq_el0, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cnthctl_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cntvoff_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(contextidr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(contextidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cpacr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cptr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(csselr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(elr_el1, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(elr_el2, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(esr_el1, u64: Esr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(esr_el2, u64: Esr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(far_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(far_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hacr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hcr_el2, u64: HcrEl2, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hpfar_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hstr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(icc_sre_el1, u64: IccSre, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(icc_sre_el2, u64: IccSre, safe_read, safe_write, fake::SYSREGS);
write_sysreg! {
    /// # Safety
    ///
    /// The SRE bit of `icc_sre_el3` must not be changed from 1 to 0, as this can result in
    /// unpredictable behaviour.
    icc_sre_el3, u64: IccSre, fake::SYSREGS
}
read_write_sysreg!(ich_hcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(ich_vmcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_sysreg!(isr_el1, u64, safe, fake::SYSREGS);
read_write_sysreg!(mair_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(mair_el2, u64, safe_read, safe_write, fake::SYSREGS);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a correct and safe configuration value for the EL3
    /// memory attribute indirection register.
    mair_el3, u64, fake::SYSREGS
}
read_write_sysreg!(mdccint_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(mdcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(mdscr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_sysreg!(midr_el1, u64, safe, fake::SYSREGS);
read_write_sysreg!(par_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(scr_el3, u64: ScrEl3, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(sctlr_el1, u64: SctlrEl1, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(sctlr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg! {
    /// # Safety
    ///
    /// Given its purpose, writing to the EL3 system control register can be very dangerous: it
    /// affects the behavior of the MMU, interrupt handling, security-relevant features like memory
    /// tagging, branch target identification, and pointer authentication, and more. Callers of
    /// `write_sctlr_el3` must ensure that the register value upholds TF-A security and reliability
    /// requirements.
    sctlr_el3, u64: SctlrEl3, safe_read, fake::SYSREGS
}
read_write_sysreg!(sp_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(sp_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(spsr_el1, u64: Spsr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(spsr_el2, u64: Spsr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tcr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a correct and safe configuration value for the EL3
    /// translation control register.
    tcr_el3, u64, fake::SYSREGS
}
read_write_sysreg!(tpidr_el0, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tpidr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tpidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tpidrro_el0, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(ttbr0_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(ttbr0_el2, u64, safe_read, safe_write, fake::SYSREGS);
write_sysreg! {
    /// # Safety
    ///
    /// The caller must ensure that `value` is a valid base address for the EL3 translation table:
    /// it must be page-aligned, and must point to a stage 1 translation table in the EL3
    /// translation regime.
    ttbr0_el3, usize, fake::SYSREGS
}
read_write_sysreg!(ttbr1_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(ttbr1_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vbar_el1, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vbar_el2, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vmpidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vpidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vtcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vttbr_el2, u64, safe_read, safe_write, fake::SYSREGS);

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct ScrEl3: u64 {
        /// RES1 bits in the `scr_el3` register.
        const RES1 = (1 << 4) | (1 << 5);
        const NS = 1 << 0;
        const IRQ = 1 << 1;
        const FIQ = 1 << 2;
        const EA = 1 << 3;
        const SMD = 1 << 7;
        const HCE = 1 << 8;
        const SIF = 1 << 9;
        const RW = 1 << 10;
        const ST = 1 << 11;
        const TWI = 1 << 12;
        const TWE = 1 << 13;
        const TLOR = 1 << 14;
        const TERR = 1 << 15;
        const APK = 1 << 16;
        const API = 1 << 17;
        const EEL2 = 1 << 18;
        const EASE = 1 << 19;
        const NMEA = 1 << 20;
        const FIEN = 1 << 21;
        const TID3 = 1 << 22;
        const TID5 = 1 << 23;
        const ENSCXT = 1 << 25;
        const ATA = 1 << 26;
        const FGTEN = 1 << 27;
        const ECVEN = 1 << 28;
        const TWEDEN = 1 << 29;
        const TME = 1 << 34;
        const AMVOFFEN = 1 << 35;
        const ENAS0 = 1 << 36;
        const ADEN = 1 << 37;
        const HXEN = 1 << 38;
        const GCSEN = 1 << 39;
        const TRNDR = 1 << 40;
        const ENTP2 = 1 << 41;
        const RCWMASKEN = 1 << 42;
        const TCR2EN = 1 << 43;
        const SCTLR2EN = 1 << 44;
        const PIEN = 1 << 45;
        const AIEN = 1 << 46;
        const D128EN = 1 << 47;
        const GPF = 1 << 48;
        const MECEN = 1 << 49;
        const ENFPM = 1 << 50;
        const TMEA = 1 << 51;
        const TWERR = 1 << 52;
        const PFAREN = 1 << 53;
        const SRMASKEN = 1 << 54;
        const ENIDCP128 = 1 << 55;
        const DSE = 1 << 57;
        const ENDSE = 1 << 58;
        const FGTEN2 = 1 << 59;
        const HDBSSEN = 1 << 60;
        const HACDBSEN = 1 << 61;
        const NSE = 1 << 62;
    }

    /// Type for the `icc_sre_el2` and `icc_sre_el3` registers.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct IccSre: u64 {
        const SRE = 1 << 0;
        const DFB = 1 << 1;
        const DIB = 1 << 2;
        const EN = 1 << 3;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SctlrEl1: u64 {
        /// RES1 bits in the `sctlr_el1` register.
        const RES1 = (1 << 29) | (1 << 28) | (1 << 23) | (1 << 22) | (1 << 20) | (1 << 11);
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct SctlrEl3: u64 {
        /// MMU enable for EL3 stage 1 address translation.
        const M = 1 << 0;
        /// Alignment check enable.
        const A = 1 << 1;
        /// Cacheability control, for data accesses at EL3.
        const C = 1 << 2;
        /// SP alignment check enable.
        const SA = 1 << 3;
        /// Cacheability control, for instruction accesses at EL3.
        const I = 1 << 12;
        /// Write permission implies XN (Execute-never). For the EL3 translation regime, this bit
        /// can force all memory regions that are writable to be treated as XN.
        const WXN = 1 << 19;
        /// RES1 bits in the `sctlr_el3` register.
        const RES1 = (1 << 23) | (1 << 18);
        const ENIB = 1 << 30;
        const ENIA = 1 << 31;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct HcrEl2: u64 {
        const TGE = 1 << 27;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
    pub struct CptrEl3: u64 {
        /// Trap EL2 accesses to CPTR_EL2/HCPTR, and EL2/EL1 accesses to CPACR_EL1/CPACR.
        const TCPAC = 1 << 31;
        /// When FEAT_AMUv1 implemented and, trap accesses from EL2/EL1/EL0 to AMU registers.
        const TAM = 1 << 30;
        /// Ttrap trace system register accesses.
        const TTA = 1 << 20;
        /// When FEAT_SME is implemented, do not trap SME instructions and system registers
        /// accesses.
        const ESM = 1 << 12;
        /// Trap Advanced SIMD instructions execution.
        const TFP = 1 << 10;
        /// Do not trap execution of SVE instructions.
        const EZ = 1 << 8;
    }
}

#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExceptionLevel {
    El0 = 0,
    El1 = 1,
    El2 = 2,
    El3 = 3,
}

/// Values for SPSEL.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StackPointer {
    El0 = 0,
    ElX = 1,
}

bitflags! {
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct Spsr: u64 {
        /// Exception was taken from AArch32 state.
        const M_EXECUTION_STATE = 1 << 4;

        /// FIQ interrupt mask.
        const F = 1 << 6;
        /// IRQ interrupt mask.
        const I = 1 << 7;
        /// SError exception mask.
        const A = 1 << 8;
        /// Debug exception mask.
        const D = 1 << 9;

        /// Illegal Execution state.
        const IL = 1 << 20;
        /// Software Step.
        const SS = 1 << 21;

        const DIT = 1 << 24;

        const V = 1 << 28;
        const C = 1 << 29;
        const Z = 1 << 30;
        const N = 1 << 31;
    }
}

impl Spsr {
    const EL_MASK: u64 = 0x3;
    const EL_SHIFT: usize = 2;
    const SP_MASK: u64 = 0x1;

    /// AArch64 execution state, EL0.
    pub const M_AARCH64_EL0: Self = Self::from_bits_retain(0b00000);
    /// AArch64 execution state, EL1 with SP_EL0.
    pub const M_AARCH64_EL1T: Self = Self::from_bits_retain(0b00100);
    /// AArch64 execution state, EL1 with SP_EL1.
    pub const M_AARCH64_EL1H: Self = Self::from_bits_retain(0b00101);
    /// AArch64 execution state, EL2 with SP_EL0.
    pub const M_AARCH64_EL2T: Self = Self::from_bits_retain(0b01000);
    /// AArch64 execution state, EL2 with SP_EL2.
    pub const M_AARCH64_EL2H: Self = Self::from_bits_retain(0b01001);
    /// AArch64 execution state, EL3 with SP_EL0.
    pub const M_AARCH64_EL3T: Self = Self::from_bits_retain(0b01100);
    /// AArch64 execution state, EL3 with SP_EL3.
    pub const M_AARCH64_EL3H: Self = Self::from_bits_retain(0b01101);

    /// Exception was taken with PSTATE.SP set to SP_EL0.
    pub const SP_EL0: Self = Self::from_bits_retain(0);
    /// Exception was taken with PSTATE.SP set to SP_ELx.
    pub const SP_ELX: Self = Self::from_bits_retain(1);

    pub const NZCV: Self = Spsr::V.union(Spsr::C).union(Spsr::Z).union(Spsr::N);

    pub const fn exception_level(self) -> ExceptionLevel {
        match (self.bits() >> Self::EL_SHIFT) & Self::EL_MASK {
            0 => ExceptionLevel::El0,
            1 => ExceptionLevel::El1,
            2 => ExceptionLevel::El2,
            3 => ExceptionLevel::El3,
            _ => unreachable!(),
        }
    }

    pub const fn stack_pointer(self) -> StackPointer {
        match self.bits() & Self::SP_MASK {
            0 => StackPointer::El0,
            1 => StackPointer::ElX,
            _ => unreachable!(),
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Eq, PartialEq)]
    pub struct Esr: u64 {
        const IL = 1 << 25;
    }
}

impl Debug for Esr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Esr({:#x})", self.0)
    }
}

impl Esr {
    pub const ISS_SYSREG_OPCODE_MASK: Self = Self::from_bits_retain(0x003f_fc1e);
}

pub fn is_feat_vhe_present() -> bool {
    const VHE: u64 = 1 << 8;

    read_id_aa64mmfr1_el1() & VHE != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_spsr() {
        assert_eq!(format!("{:?}", Spsr::empty()), "Spsr(0x0)");
        assert_eq!(format!("{:?}", Spsr::NZCV), "Spsr(V | C | Z | N)");
        assert_eq!(format!("{:?}", Spsr::M_AARCH64_EL3H), "Spsr(0xd)");
    }

    #[test]
    fn debug_esr() {
        assert_eq!(format!("{:?}", Esr::empty()), "Esr(0x0)");
        assert_eq!(format!("{:?}", Esr::IL), "Esr(0x2000000)");
        assert_eq!(
            format!("{:?}", Esr::ISS_SYSREG_OPCODE_MASK),
            "Esr(0x3ffc1e)"
        );
    }
}
