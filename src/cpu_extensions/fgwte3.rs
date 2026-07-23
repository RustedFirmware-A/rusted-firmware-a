// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fine-grained write traps support for EL3.
//!
//! Traps write accesses at EL3 to individual `*_EL3` System registers.

use arm_sysregs::{
    el1::accessors::read_id_aa64mmfr4_el1,
    el3::{accessors::write_fgwte3_el3, registers::Fgwte3El3},
};

/// Indicates whether TPIDR_EL3 is used in crash reporting.
const FGWTE3_TPIDR_CRASH_REPORTING: bool = const {
    #[cfg(not(any(test, feature = "fakes")))]
    {
        crate::debug::CRASH_REPORTING
    }

    #[cfg(any(test, feature = "fakes"))]
    {
        false
    }
};

/// The desired `FGWTE3_EL3` value after initializing all the affected EL3 registers. After writing
/// this value to `FGWTE3_EL3`, the registers denoted by the fields will trap to EL3 when written
/// to. The decision whether to set these bits might be revised when new features are added.
///
/// Locking registers for CPU extensions which are unsupported will have no effect.
///
/// **Summary on the decisions about the fields of `FGWTE3_EL3`:**
///
/// Writes to the following registers are disabled:
///  - `ACTLR_EL3`, `AFSR0_EL3`, `AFSR1_EL3`, `AMAIR_EL3`, `AMAIR2_EL3`: Not used.
///  - `GCSCR_EL3`, `GCSPR_EL3`: used by `FEAT_GCS`. Not used.
///  - `GPCCR_EL3`, `GPTBR_EL3`: used by `FEAT_RME`. Written to only when enabling Granule
///    Protection checks.
///  - `MAIR_EL3`: set only in `enable_mmu`.
///  - `MAIR2_EL3`: used by `FEAT_AIE`. Not supported.
///  - `MECID_RL_A_EL3`: used by `FEAT_MEC`. Not supported.
///  - `PIR_EL3`: used by `FEAT_S1PIE`. Not supported.
///  - `SCTLR2_EL3`: used by `FEAT_SCTLR2`. Set only during boot in `init_sctlr2_el3`, and
///    `pauth::init`, if PAuth is enabled.
///  - `SPMROOTCR_EL3`: used by `FEAT_SPMU`. Not supported.
///  - `TCR_EL3`:  set only in `enable_mmu`.
///  - `TTBR0_EL3`: set only during boot in `enable_mmu` and `init_runtime_mapping`.
///  - `TPIDR_EL3`: written to on exceptions and crashes, when CRASH_REPORTING is true.
///  - `VBAR_EL3`: only set in the entrypoint.
///  - `GPCBW_EL3`: used by `FEAT_RME_GPC3`. Not supported.
///
/// The following registers are intentionally left writable:
///  - `MDCR_EL3`: context switched register.
///  - `MPAM3_EL3`: context switched register.
///  - `SCTLR_EL3`: required to be written to on MMU powerdown.
const FGWTE3_INIT_VAL: Fgwte3El3 = Fgwte3El3::ACTLR_EL3
    .union(Fgwte3El3::AFSR0_EL3)
    .union(Fgwte3El3::AFSR1_EL3)
    .union(Fgwte3El3::AMAIR_EL3)
    .union(Fgwte3El3::AMAIR2_EL3)
    .union(Fgwte3El3::GCSCR_EL3)
    .union(Fgwte3El3::GCSPR_EL3)
    .union(Fgwte3El3::GPCCR_EL3)
    .union(Fgwte3El3::GPTBR_EL3)
    .union(Fgwte3El3::MAIR_EL3)
    .union(Fgwte3El3::MAIR2_EL3)
    .union(Fgwte3El3::MECID_RL_A_EL3)
    .union(Fgwte3El3::PIR_EL3)
    .union(Fgwte3El3::SCTLR2_EL3)
    .union(Fgwte3El3::SPMROOTCR_EL3)
    .union(Fgwte3El3::TCR_EL3)
    .union(Fgwte3El3::TTBR0_EL3)
    .union(Fgwte3El3::VBAR_EL3)
    .union(Fgwte3El3::GPCBW_EL3)
    .union(if FGWTE3_TPIDR_CRASH_REPORTING {
        Fgwte3El3::empty()
    } else {
        Fgwte3El3::TPIDR_EL3
    });

/// Writes to `FGWTE3_EL3` if FEAT_FGWTE3 is supported, making writes to a set of EL3 registers trap
/// to EL3. Assumes the registers to be locked have already been initialized.
pub fn disable_el3_register_writes() {
    if read_id_aa64mmfr4_el1().is_feat_fgwte3_present() {
        // SAFETY: FEAT_FGWTE3 is present. `FGWTE3_INIT_VAL` is valid.
        unsafe {
            write_fgwte3_el3(FGWTE3_INIT_VAL);
        }
    }
}
