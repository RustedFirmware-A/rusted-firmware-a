// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Access to Arm CPU system registers.

#![cfg_attr(not(any(test, feature = "fakes")), no_std)]

#[cfg(not(any(test, feature = "fakes")))]
mod aarch64;
#[cfg(any(test, feature = "fakes"))]
pub mod fake;

#[doc(hidden)]
pub use paste as _paste;

use bitflags::bitflags;
use core::fmt::{self, Debug, Formatter};

/// Generates public functions named `read_$sysreg` and `write_$sysreg` to read or write
/// (respectively) a value of type `$type` from/to the system register `$sysreg`.
///
/// `safe_read` and `safe_write` should only be specified for system registers which are indeed safe
/// to read from or write any value to.
#[macro_export]
macro_rules! read_write_sysreg {
    ($sysreg:ident $(: $asm_sysreg:ident)?, $type:ty $(: $bitflags_type:ty)?, safe_read, safe_write $(, $fake_sysregs:expr)?) => {
        $crate::read_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
        $crate::write_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
    };
    (
        $(#[$attributes:meta])*
        $sysreg:ident $(: $asm_sysreg:ident)?, $type:ty $(: $bitflags_type:ty)?, safe_read $(, $fake_sysregs:expr)?
    ) => {
        $crate::read_sysreg!($sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)?, safe $(, $fake_sysregs)?);
        $crate::write_sysreg! {
            $(#[$attributes])*
            $sysreg $(: $asm_sysreg)?, $type $(: $bitflags_type)? $(, $fake_sysregs)?
        }
    };
}

bitflags! {
    /// ID_AA64MMFR1_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct IdAa64mmfr1El1: u64 {}
}

impl IdAa64mmfr1El1 {
    const VH_SHIFT: u64 = 8;
    const VH_MASK: u64 = 0b1111;
    const VH_SUPPORTED: u64 = 0b0001;

    const HCX_SHIFT: u64 = 40;
    const HCX_MASK: u64 = 0b1111;
    const HCX_SUPPORTED: u64 = 0b0001;

    /// Indicates presence of FEAT_VHE.
    pub fn is_feat_vhe_present(self) -> bool {
        (self.bits() >> Self::VH_SHIFT) & Self::VH_MASK >= Self::VH_SUPPORTED
    }

    /// Indicates presence of FEAT_HCX.
    pub fn is_feat_hcx_present(self) -> bool {
        (self.bits() >> Self::HCX_SHIFT) & Self::HCX_MASK >= Self::HCX_SUPPORTED
    }
}

bitflags! {
    /// ID_AA64MMFR2_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct IdAa64mmfr2El1: u64 { }
}

impl IdAa64mmfr2El1 {
    const CCIDX_SHIFT: u64 = 20;
    const CCIDX_MASK: u64 = 0b1111;
    const CCIDX_64_BIT: u64 = 0b0001;

    /// Checks whether 64-bit format is implemented for all levels of the CCSIDR_EL1.
    pub fn has_64_bit_ccsidr_el1(self) -> bool {
        (self.bits() >> Self::CCIDX_SHIFT) & Self::CCIDX_MASK == Self::CCIDX_64_BIT
    }
}

bitflags! {
    /// ID_AA64MMFR3_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct IdAa64mmfr3El1: u64 {}
}

impl IdAa64mmfr3El1 {
    const TCRX_SHIFT: u64 = 0;
    const TCRX_MASK: u64 = 0b1111;
    const TCRX_SUPPORTED: u64 = 1;

    /// Indicates presence of FEAT_TCR2.
    pub fn is_feat_tcr2_present(self) -> bool {
        (self.bits() >> Self::TCRX_SHIFT) & Self::TCRX_MASK >= Self::TCRX_SUPPORTED
    }
}

bitflags! {
    /// MPIDR_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct MpidrEl1: u64 {
        /// MT
        const MT = 1 << 24;
        /// U
        const U = 1 << 30;
    }
}

impl MpidrEl1 {
    /// Mask for the Aff0 field.
    pub const AFF0_MASK: u64 = 0xff << Self::AFF0_SHIFT;
    /// Mask for the Aff1 field.
    pub const AFF1_MASK: u64 = 0xff << Self::AFF1_SHIFT;
    /// Mask for the Aff2 field.
    pub const AFF2_MASK: u64 = 0xff << Self::AFF2_SHIFT;
    /// Mask for the Aff3 field.
    pub const AFF3_MASK: u64 = 0xff << Self::AFF3_SHIFT;
    /// Size in bits of the affinity fields.
    pub const AFFINITY_BITS: usize = 8;
    /// Position of the lowest bit in the Aff0 field.
    pub const AFF0_SHIFT: u8 = 0;
    /// Position of the lowest bit in the Aff1 field.
    pub const AFF1_SHIFT: u8 = 8;
    /// Position of the lowest bit in the Aff2 field.
    pub const AFF2_SHIFT: u8 = 16;
    /// Position of the lowest bit in the Aff3 field.
    pub const AFF3_SHIFT: u8 = 32;

    /// Converts a PSCI MPIDR value into the equivalent `MpidrEL1` value.
    ///
    /// This reads the MT and U bits from the current CPU's MPIDR_EL1 value and combines them with
    /// the affinity values from the given `psci_mpidr`.
    ///
    /// This assumes that the MPIDR_EL1 values of all CPUs in a system have the same values for the
    /// MT and U bits.
    pub fn from_psci_mpidr(psci_mpidr: u64) -> Self {
        let mpidr_el1 = read_mpidr_el1();
        Self::from_bits_retain(psci_mpidr) | (mpidr_el1 & (Self::MT | Self::U))
    }

    /// Returns the value of the Aff0 field.
    pub fn aff0(self) -> u8 {
        (self.bits() >> Self::AFF0_SHIFT) as u8
    }

    /// Returns the value of the Aff1 field.
    pub fn aff1(self) -> u8 {
        (self.bits() >> Self::AFF1_SHIFT) as u8
    }

    /// Returns the value of the Aff2 field.
    pub fn aff2(self) -> u8 {
        (self.bits() >> Self::AFF2_SHIFT) as u8
    }

    /// Returns the value of the Aff3 field.
    pub fn aff3(self) -> u8 {
        (self.bits() >> Self::AFF3_SHIFT) as u8
    }
}

/// Cache type enum.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CacheType {
    /// No cache.
    NoCache = 0b000,
    /// Instruction cache only.
    InstructionOnly = 0b001,
    /// Data cache only.
    DataOnly = 0b010,
    /// Separate instruction and data caches.
    SeparateInstructionAndData = 0b011,
    /// Unified cache.
    Unified = 0b100,
}

impl TryFrom<u64> for CacheType {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(match value {
            0b000 => Self::NoCache,
            0b001 => Self::InstructionOnly,
            0b010 => Self::DataOnly,
            0b011 => Self::SeparateInstructionAndData,
            0b100 => Self::Unified,
            _ => return Err(()),
        })
    }
}

/// Wrapper type for describing cache level in a human readable format, i.e. L3 cache = `CacheLevel(3)`
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CacheLevel(u8);

impl CacheLevel {
    /// Creates new instance.
    pub fn new(level: u8) -> Self {
        assert!((1..8).contains(&level));
        Self(level)
    }

    /// Returns the level value.
    pub fn level(&self) -> u8 {
        self.0
    }
}

impl From<CacheLevel> for u64 {
    fn from(value: CacheLevel) -> Self {
        (value.0 - 1).into()
    }
}

bitflags! {
    /// CLIDR_EL1, Cache Level ID Register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct ClidrEl1: u64 { }
}

impl ClidrEl1 {
    const LEVEL_MASK: u64 = 0b111;
    const ICB_SHIFT: u64 = 30;
    const LOUU_SHIFT: u64 = 27;
    const LOC_SHIFT: u64 = 24;
    const LOUIS_SHIFT: u64 = 21;
    const CTYPE_SHIFT: u64 = 3;

    /// Returns the inner cache boundary level.
    pub fn icb(self) -> Option<CacheLevel> {
        let icb = (self.bits() >> Self::ICB_SHIFT) & Self::LEVEL_MASK;
        if icb != 0 {
            Some(CacheLevel(icb as u8))
        } else {
            None
        }
    }

    /// Return level of Unification Uniprocessor for the cache hierarchy.
    pub fn louu(self) -> u64 {
        (self.bits() >> Self::LOUU_SHIFT) & Self::LEVEL_MASK
    }

    /// Returns level of Coherence for the cache hierarchy.
    pub fn loc(self) -> u64 {
        (self.bits() >> Self::LOC_SHIFT) & Self::LEVEL_MASK
    }

    /// Returns level of Unification Inner Shareable for the cache hierarchy.
    pub fn louis(self) -> u64 {
        (self.bits() >> Self::LOUIS_SHIFT) & Self::LEVEL_MASK
    }

    /// Returns Cache Type [1-7] fields.
    pub fn ctype(self, level: CacheLevel) -> CacheType {
        let shift = Self::CTYPE_SHIFT * u64::from(level);
        ((self.bits() >> shift) & Self::LEVEL_MASK)
            .try_into()
            .unwrap()
    }
}

bitflags! {
    /// CSSELR_EL1, Cache Size Selection Register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub struct CsselrEl1: u64 {
        /// Allocation Tag not Data bit, only valid if FEAT_MTE2 is implemented.
        const TND = 1 << 4;
        /// Instruction not Data bit.
        const IND = 1 << 0;
    }
}

impl CsselrEl1 {
    const LEVEL_MASK: u64 = 0b111;
    const LEVEL_SHIFT: u64 = 1;

    /// Creates new instance. TnD is only valid if FEAT_MTE2 is implemented.
    pub fn new(tnd: bool, level: CacheLevel, ind: bool) -> Self {
        let mut instance = Self::from_bits_retain(u64::from(level) << Self::LEVEL_SHIFT);

        if ind {
            instance |= Self::IND;
        } else if tnd {
            // TnD is only valid if InD is not set.
            instance |= Self::TND;
        }

        instance
    }

    /// Returns the cache level of requested cache.
    pub fn level(self) -> CacheLevel {
        CacheLevel(((self.bits() >> Self::LEVEL_SHIFT) & Self::LEVEL_MASK) as u8 + 1)
    }
}

bitflags! {
    /// CTR_EL0, Cache Type Register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct CtrEl0: u64 {}
}

impl CtrEl0 {
    /// Log2 of the number of words in the smallest cache line of all the data caches and unified
    /// caches that are controlled by the PE.
    pub fn dminline(self) -> usize {
        ((self.bits() >> 16) & 0xf) as usize
    }
}

bitflags! {
    /// SCR_EL3 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct ScrEl3: u64 {
        /// RES1 bits in the `scr_el3` register.
        const RES1 = (1 << 4) | (1 << 5);
        /// Non-secure.
        const NS = 1 << 0;
        /// Take physical IRQs at EL3.
        const IRQ = 1 << 1;
        /// Take physical FIQs at EL3.
        const FIQ = 1 << 2;
        /// Take external abort and SError exceptions at EL3.
        const EA = 1 << 3;
        /// Disable SMC instructions.
        const SMD = 1 << 7;
        /// Enable HVC instructions.
        const HCE = 1 << 8;
        /// Disable execution from non-secure memory.
        const SIF = 1 << 9;
        /// Enable AArch64 in lower ELs.
        const RW = 1 << 10;
        /// Trap physical secure timer to EL3.
        const ST = 1 << 11;
        /// Trap WFI to EL3.
        const TWI = 1 << 12;
        /// Trap WFE to EL3.
        const TWE = 1 << 13;
        /// Trap LOR register access to EL3.
        const TLOR = 1 << 14;
        /// Trap error record register access to EL3.
        const TERR = 1 << 15;
        /// Don't trap PAC key registers to EL3.
        const APK = 1 << 16;
        /// Don't trap PAuth instructions to EL3.
        const API = 1 << 17;
        /// Enable Secure EL2.
        const EEL2 = 1 << 18;
        /// Synchronous external aborts are taken as SErrors.
        const EASE = 1 << 19;
        /// Take SError exceptions at EL3.
        const NMEA = 1 << 20;
        /// Enable fault injection at lower ELs.
        const FIEN = 1 << 21;
        /// Trap ID group 3 registers to EL3.
        const TID3 = 1 << 22;
        /// Trap ID group 5 register to EL3.
        const TID5 = 1 << 23;
        /// Enable SCXT at lower ELs.
        const ENSCXT = 1 << 25;
        /// Enable memory tagging at lower ELs.
        const ATA = 1 << 26;
        /// Enable fine-grained traps to EL2.
        const FGTEN = 1 << 27;
        /// Enable access to CNTPOFF_EL2.
        const ECVEN = 1 << 28;
        /// Enable a configurable delay for WFE traps.
        const TWEDEN = 1 << 29;
        /// Enable access to TME at lower ELs.
        const TME = 1 << 34;
        /// Enable acivity monitors virtual offsets.
        const AMVOFFEN = 1 << 35;
        /// Enable ST64BV0 at lower ELs.
        const ENAS0 = 1 << 36;
        /// Enable ACCDATA_EL1 at lower ELs.
        const ADEN = 1 << 37;
        /// Enable HCRX_EL2.
        const HXEN = 1 << 38;
        /// Enable gaurded control stack.
        const GCSEN = 1 << 39;
        /// Trap RNDR and RNDRRS to EL3.
        const TRNDR = 1 << 40;
        /// Enable TPIDR2_EL0 at lower ELs.
        const ENTP2 = 1 << 41;
        /// Enable RCW and RCWS mask registers at lower ELs.
        const RCWMASKEN = 1 << 42;
        /// Enable TCR2_ELx registers at lower ELs.
        const TCR2EN = 1 << 43;
        /// Enable SCTLR2_ELx rogisters at lower ELs.
        const SCTLR2EN = 1 << 44;
        /// Enable permission indirection and overlay registers at lower ELs.
        const PIEN = 1 << 45;
        /// Enable MAIR2_ELx and AMAIR2_ELx at lower ELs.
        const AIEN = 1 << 46;
        /// Enable 128-bit system registers at  lower ELs.
        const D128EN = 1 << 47;
        /// Route GPFs to EL3.
        const GPF = 1 << 48;
        /// Enable MECID registers at EL2.
        const MECEN = 1 << 49;
        /// Enable access to FPMR at lower ELs.
        const ENFPM = 1 << 50;
        /// Take synchronous external abort and physical SError exception to EL3.
        const TMEA = 1 << 51;
        /// Trap writes to Error Record registers to EL3.
        const TWERR = 1 << 52;
        /// Enable access to physical fault address registers at lower ELs.
        const PFAREN = 1 << 53;
        /// Enable access to mask registers at lower ELs.
        const SRMASKEN = 1 << 54;
        /// Enable implementation-defined 128-bit system registers.
        const ENIDCP128 = 1 << 55;
        /// A delegated SError exception is pending.
        const DSE = 1 << 57;
        /// Enable delegated SError exceptions.
        const ENDSE = 1 << 58;
        /// Enable fine-grained traps to EL2.
        const FGTEN2 = 1 << 59;
        /// Enable HDBSSBR_EL2 and HDBSSPROD_EL2 registers at EL2.
        const HDBSSEN = 1 << 60;
        /// Enable HACDBSBR_EL2 and HACDBSCONS_EL2 registers at EL2.
        const HACDBSEN = 1 << 61;
        /// Non-secure realm world bit.
        const NSE = 1 << 62;
    }

    /// Type for the `icc_sre_el2` and `icc_sre_el3` registers.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct IccSre: u64 {
        /// Enable the system register interface.
        const SRE = 1 << 0;
        /// Disable FIQ bypass.
        const DFB = 1 << 1;
        /// Disable IRQ bypass.
        const DIB = 1 << 2;
        /// Enable lower exception level access.
        const EN = 1 << 3;
    }

    /// SCTLR_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct SctlrEl1: u64 {
        /// RES1 bits in the `sctlr_el1` register.
        const RES1 = (1 << 29) | (1 << 28) | (1 << 23) | (1 << 22) | (1 << 20) | (1 << 11);
        /// Do not set Privileged Access Never, on taking an exception to EL1.
        const SPAN = 1 << 23;
    }

    /// SCTLR_EL2 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct SctlrEl2: u64 {
        /// Do not set Privileged Access Never, on taking an exception to EL2.
        const SPAN = 1 << 23;
    }

    /// SCTLR_EL3 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
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
        /// Enable Implicit Error Synchronization events.
        const IESB = 1 << 21;
        /// RES1 bits in the `sctlr_el3` register.
        const RES1 = (1 << 23) | (1 << 18);
        /// Enable pointer authentication using APIBKey_EL1.
        const ENIB = 1 << 30;
        /// Enable pointer authentication using APIAKey_EL1.
        const ENIA = 1 << 31;
    }

    /// HCR_EL2 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct HcrEl2: u64 {
        /// Trap general exceptions to EL2.
        const TGE = 1 << 27;
    }

    /// HCRX_EL2 - Extended Hypervisor Configuration Register.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct HcrxEl2: u64 {
        /// Do not trap execution of an ST64BV0 instruction at EL0 or EL1 to EL2.
        const EnAS0 = 1 << 0;
        /// Do not trap execution of an LD64B or ST64B instruction at EL0 or EL1 to EL2.
        const EnALS = 1 << 1;
        /// Do not trap execution of an ST64BV instruction at EL0 or EL1 to EL2.
        const EnASR = 1 << 2;
        /// Determines the behavior of TLBI instructions affected by the XS attribute.
        const FnXS = 1 << 3;
        /// Determines if the fine-grained traps in HFGITR_EL2 also apply to the corresponding TLBI
        /// maintenance instructions with the nXS qualifier.
        const FGTnXS = 1 << 4;
        /// Controls mapping of the value of SMPRI_EL1.Priority for streaming execution priority at
        /// EL0 or EL1.
        const SMPME = 1 << 5;
        /// Traps MSR writes of ALLINT at EL1 using AArch64 to EL2.
        const TALLINT = 1 << 6;
        /// Enables signaling of virtual IRQ interrupts with Superpriority.
        const VINMI = 1 << 7;
        /// Enables signaling of virtual FIQ interrupts with Superpriority.
        const VFNMI = 1 << 8;
        /// Controls the required permissions for cache maintenance instructions at EL1 or EL0.
        const CMOW = 1 << 9;
        /// Controls Memory Copy and Memory Set exceptions generated from EL1.
        const MCE2 = 1 << 10;
        /// Enables execution of Memory Set and Memory Copy instructions at EL1 or EL0.
        const MSCEn = 1 << 11;
    }

    /// CPTR_EL3 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
    #[repr(transparent)]
    pub struct CptrEl3: u64 {
        /// Trap EL2 accesses to CPTR_EL2/HCPTR, and EL2/EL1 accesses to CPACR_EL1/CPACR.
        const TCPAC = 1 << 31;
        /// When FEAT_AMUv1 implemented and, trap accesses from EL2/EL1/EL0 to AMU registers.
        const TAM = 1 << 30;
        /// Trap trace system register accesses.
        const TTA = 1 << 20;
        /// When FEAT_SME is implemented, do not trap SME instructions and system registers
        /// accesses.
        const ESM = 1 << 12;
        /// Trap Advanced SIMD instructions execution.
        const TFP = 1 << 10;
        /// Do not trap execution of SVE instructions.
        const EZ = 1 << 8;
    }

    /// PMCR_EL0 register configures and controls the Performance Monitors counters.
    #[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
    #[repr(transparent)]
    pub struct Pmcr: u64 {
        /// Enable. Affected counters are enabled by PMCNTENSET_EL0.
        const E = 1 << 0;
        /// Event counter reset. Reset all affected event counters PMEVCNTR<n>_EL0 to zero.
        const P = 1 << 1;
        /// Cycle counter reset. Reset PMCCNTR_EL0 to zero.
        const C = 1 << 2;
        /// Clock divider. If set PMCCNTR_EL0 counts once every 64 clock cycles.
        const D = 1 << 3;
        /// Enable export of events in an IMPLEMENTATION DEFINED PMU event export bus. If set,
        /// export events where not prohibited.
        const X = 1 << 4;
        /// If set, cycle counting by PMCCNTR_EL0 is disabled in prohibited regions.
        const DP = 1 << 5;
    }

    /// MDCR_EL2 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct MdcrEl2: u64 {}

    /// MDCR_EL3 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct MdcrEl3: u64 {
        /// Realm Trace enable. Enables tracing in Realm state.
        const RLTE = 1 << 0;
        /// Trap Performance Monitor register accesses
        const TPM = 1 << 6;
        /// Do not trap various PMUv3p9 related system register accesses to EL3.
        const ENPM2 = 1 << 7;
        /// Secure Performance Monitors Enable. Controls event counting in Secure state and EL3.
        const SPME = 1 << 17;
        /// Secure Trace enable. Enables tracing in Secure state.
        const STE = 1 << 18;
        /// Trap Trace Filter controls. Traps use of the Trace Filter control registers at EL2 and
        /// EL1 to EL3.
        const TTRF = 1 << 19;
        /// Secure Cycle Counter Disable. Prohibits PMCCNTR_EL0 from counting in Secure state.
        const SCCD = 1 << 23;
        /// Enable TRBE register access for the security state that owns the buffer.
        const NSTB_EN = 1 << 24;
        /// Together with MDCR_EL3.NSTBE determines which security state owns the trace buffer
        const NSTB_SS = 1 << 25;
        /// Non-secure Trace Buffer Extended. Together with MDCR_EL3.NSTB, controls the trace
        /// buffer owning Security state and accesses to trace buffer System registers from EL2
        /// and EL1.
        const NSTBE = 1 << 26;
        /// Multi-threaded PMU Enable. Enables use of the PMEVTYPER<n>_EL0.MT bits.
        const MTPME = 1 << 28;
        /// Monitor Cycle Counter Disable. Prohibits the Cycle Counter, PMCCNTR_EL0, from counting
        /// at EL3.
        const MCCD = 1 << 34;
        /// Monitor Performance Monitors Extended control. In conjunction with MDCR_EL3.SPME,
        /// controls when event counters are enabled at EL3 and in other Secure Exception levels.
        const MPMX = 1 << 35;
    }
}

/// An AArch64 exception level.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExceptionLevel {
    /// Exception level 0.
    El0 = 0,
    /// Exception level 1.
    El1 = 1,
    /// Exception level 2.
    El2 = 2,
    /// Exception level 3.
    El3 = 3,
}

/// Values for SPSEL.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum StackPointer {
    /// Use SP_EL0.
    El0 = 0,
    /// Use SP_EL1, SP_EL2 or SP_EL3 according to the current exception level.
    ElX = 1,
}

bitflags! {
    /// SPSR_ELn system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
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
        /// Privileged Access Never.
        const PAN = 1 << 22;
        /// Data independent timing.
        const DIT = 1 << 24;

        /// Overflow condition flag.
        const V = 1 << 28;
        /// Carry condition flag.
        const C = 1 << 29;
        /// Zero condition flag.
        const Z = 1 << 30;
        /// Negative condition flag.
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

    /// All of the N, Z, C and V bits.
    pub const NZCV: Self = Spsr::V.union(Spsr::C).union(Spsr::Z).union(Spsr::N);

    /// Returns the value of the EL field.
    pub const fn exception_level(self) -> ExceptionLevel {
        match (self.bits() >> Self::EL_SHIFT) & Self::EL_MASK {
            0 => ExceptionLevel::El0,
            1 => ExceptionLevel::El1,
            2 => ExceptionLevel::El2,
            3 => ExceptionLevel::El3,
            _ => unreachable!(),
        }
    }

    /// Returns the value of the SP field.
    pub const fn stack_pointer(self) -> StackPointer {
        match self.bits() & Self::SP_MASK {
            0 => StackPointer::El0,
            1 => StackPointer::ElX,
            _ => unreachable!(),
        }
    }
}

bitflags! {
    /// ESR_ELn value.
    #[derive(Clone, Copy, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct Esr: u64 {
        /// 32-bit instruction length.
        const IL = 1 << 25;
    }
}

impl Esr {
    /// Mask for the parts of an ESR value containing the opcode.
    pub const ISS_SYSREG_OPCODE_MASK: Self = Self::from_bits_retain(0x003f_fc1e);
}

impl Debug for Esr {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Esr({:#x})", self.0)
    }
}

bitflags! {
    /// ID_AA64DFR0_EL1 system register value.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    #[repr(transparent)]
    pub struct IdAa64dfr0El1: u64 {}
}

impl IdAa64dfr0El1 {
    const TRACE_VER_SHIFT: u64 = 4;
    const TRACE_VER_MASK: u64 = 0b1111;
    const SYS_REG_TRACE_SUPPORTED: u64 = 1;

    const TRACE_FILT_SHIFT: u64 = 40;
    const TRACE_FILT_MASK: u64 = 0b1111;
    const TRF_SUPPORTED: u64 = 1;

    const TRACE_BUFFER_SHIFT: u64 = 44;
    const TRACE_BUFFER_MASK: u64 = 0b1111;
    const TRBE_NOT_SUPPORTED: u64 = 0;

    const MTPMU_SHIFT: u64 = 48;
    const MTPMU_MASK: u64 = 0b1111;
    const MTPMU_SUPPORTED: u64 = 1;

    /// Trace support. Indicates whether System register interface to a PE trace unit is
    /// implemented.
    pub fn is_feat_sys_reg_trace_present(self) -> bool {
        (self.bits() >> Self::TRACE_VER_SHIFT) & Self::TRACE_VER_MASK
            == Self::SYS_REG_TRACE_SUPPORTED
    }

    /// Indicates whether Armv8.4 Self-hosted Trace Extension is implemented.
    pub fn is_feat_trf_present(self) -> bool {
        (self.bits() >> Self::TRACE_FILT_SHIFT) & Self::TRACE_FILT_MASK == Self::TRF_SUPPORTED
    }

    /// Indicates whether Trace Buffer Extension is implemented.
    pub fn is_feat_trbe_present(self) -> bool {
        (self.bits() >> Self::TRACE_BUFFER_SHIFT) & Self::TRACE_BUFFER_MASK
            != Self::TRBE_NOT_SUPPORTED
    }

    /// Indicates whether Multi Threaded PMU Extension is implemented.
    pub fn is_feat_mtpmu_present(self) -> bool {
        (self.bits() >> Self::MTPMU_SHIFT) & Self::MTPMU_MASK == Self::MTPMU_SUPPORTED
    }
}

read_sysreg!(id_aa64dfr0_el1, u64: IdAa64dfr0El1, safe, fake::SYSREGS);
read_sysreg!(id_aa64mmfr1_el1, u64: IdAa64mmfr1El1, safe, fake::SYSREGS);
read_sysreg!(id_aa64mmfr2_el1, u64: IdAa64mmfr2El1, safe, fake::SYSREGS);
read_sysreg!(id_aa64mmfr3_el1, u64: IdAa64mmfr3El1, safe, fake::SYSREGS);
read_sysreg!(mpidr_el1, u64: MpidrEl1, safe, fake::SYSREGS);
read_write_sysreg!(actlr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(actlr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr0_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr0_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr1_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(afsr1_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(amair_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(amair_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_sysreg!(ccsidr_el1, u64, safe, fake::SYSREGS);
read_sysreg!(clidr_el1, u64: ClidrEl1, safe, fake::SYSREGS);
read_write_sysreg!(cntfrq_el0, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cnthctl_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cntvoff_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(contextidr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(contextidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cpacr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(cptr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(csselr_el1, u64: CsselrEl1, safe_read, safe_write, fake::SYSREGS);
read_sysreg!(ctr_el0, u64: CtrEl0, safe, fake::SYSREGS);
read_write_sysreg!(disr_el1: s3_0_c12_c1_1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(elr_el1, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(elr_el2, usize, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(esr_el1, u64: Esr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(esr_el2, u64: Esr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(far_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(far_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hacr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hcr_el2, u64: HcrEl2, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(hcrx_el2: s3_4_c1_c2_2, u64: HcrxEl2, safe_read, safe_write, fake::SYSREGS);
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
read_write_sysreg!(mdcr_el2, u64: MdcrEl2, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(mdscr_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_sysreg!(midr_el1, u64, safe, fake::SYSREGS);
read_write_sysreg!(par_el1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(pmcr_el0, u64: Pmcr, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(scr_el3, u64: ScrEl3, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(sctlr_el1, u64: SctlrEl1, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(sctlr_el2, u64: SctlrEl2, safe_read, safe_write, fake::SYSREGS);
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
read_write_sysreg!(tcr2_el1: s3_0_c2_c0_3, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(tcr2_el2: s3_4_c2_c0_3, u64, safe_read, safe_write, fake::SYSREGS);
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
read_write_sysreg!(vdisr_el2: s3_4_c12_c1_1, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vmpidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vpidr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vsesr_el2: s3_4_c5_c2_3, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vtcr_el2, u64, safe_read, safe_write, fake::SYSREGS);
read_write_sysreg!(vttbr_el2, u64, safe_read, safe_write, fake::SYSREGS);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_mpidr_el1() {
        assert_eq!(format!("{:?}", MpidrEl1::empty()), "MpidrEl1(0x0)");
        assert_eq!(
            format!("{:?}", MpidrEl1::MT | MpidrEl1::U),
            "MpidrEl1(MT | U)"
        );
        assert_eq!(
            format!("{:?}", MpidrEl1::from_bits_retain(0x12_4134_5678)),
            "MpidrEl1(MT | U | 0x1200345678)"
        );
    }

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
