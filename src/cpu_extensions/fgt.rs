// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(any(feature = "sel2", feature = "rme"))]
use crate::context::World;
use crate::cpu_extensions::CpuExtension;
use arm_sysregs::{HfgitrEl2, HfgrtrEl2, HfgwtrEl2};
#[cfg(not(any(feature = "sel2", feature = "rme")))]
use arm_sysregs::{write_hfgitr_el2, write_hfgrtr_el2, write_hfgwtr_el2};

// Initialization values for the HFG*_EL2 registers that disable some fine-grained traps so that
// legacy systems unaware of FEAT_FGT do not get trapped due to their lack of initialization for
// this feature.
// Note: These values are aligned to the same definitions in TF-A, but leave some of the traps
// enabled.
// TODO: Evaluate if the remaining traps should be disabled by default, or alternatively remove this
// initialization if EL2 systems now reliably initialize these registers.
const HFGITR_EL2_INIT_VAL: HfgitrEl2 = HfgitrEl2::NBRBINJ.union(HfgitrEl2::NBRBIALL);
const HFGRTR_EL2_INIT_VAL: HfgrtrEl2 = HfgrtrEl2::NACCDATA_EL1
    .union(HfgrtrEl2::NSMPRI_EL1)
    .union(HfgrtrEl2::NTPIDR2_EL0);
const HFGWTR_EL2_INIT_VAL: HfgwtrEl2 = HfgwtrEl2::NACCDATA_EL1
    .union(HfgwtrEl2::NSMPRI_EL1)
    .union(HfgwtrEl2::NTPIDR2_EL0);

#[cfg(any(feature = "sel2", feature = "rme"))]
mod fgt_el2 {
    use crate::{
        context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
        platform::{Platform, PlatformImpl, exception_free},
    };
    use arm_sysregs::{
        HafgrtrEl2, HdfgrtrEl2, HdfgwtrEl2, HfgitrEl2, HfgrtrEl2, HfgwtrEl2, read_hafgrtr_el2,
        read_hdfgrtr_el2, read_hdfgwtr_el2, read_hfgitr_el2, read_hfgrtr_el2, read_hfgwtr_el2,
        read_id_aa64pfr0_el1, write_hafgrtr_el2, write_hdfgrtr_el2, write_hdfgwtr_el2,
        write_hfgitr_el2, write_hfgrtr_el2, write_hfgwtr_el2,
    };
    use core::cell::RefCell;
    use percore::{ExceptionLock, PerCore};

    struct FgtCpuContext {
        hafgrtr_el2: HafgrtrEl2,
        hdfgrtr_el2: HdfgrtrEl2,
        hdfgwtr_el2: HdfgwtrEl2,
        hfgitr_el2: HfgitrEl2,
        hfgrtr_el2: HfgrtrEl2,
        hfgwtr_el2: HfgwtrEl2,
    }

    impl FgtCpuContext {
        const INIT_VAL: Self = Self {
            hafgrtr_el2: HafgrtrEl2::empty(),
            hdfgrtr_el2: HdfgrtrEl2::empty(),
            hdfgwtr_el2: HdfgwtrEl2::empty(),
            // Initialize the FGT context with the HFG*_EL2 init values so that they get restored on
            // first entry to a world.
            hfgitr_el2: super::HFGITR_EL2_INIT_VAL,
            hfgrtr_el2: super::HFGRTR_EL2_INIT_VAL,
            hfgwtr_el2: super::HFGWTR_EL2_INIT_VAL,
        };
    }

    static FGT_CTX: PerCoreState<PerWorld<FgtCpuContext>> = PerCore::new(
        [const {
            ExceptionLock::new(RefCell::new(PerWorld(
                [FgtCpuContext::INIT_VAL; CPU_DATA_CONTEXT_NUM],
            )))
        }; PlatformImpl::CORE_COUNT],
    );

    pub fn save_context(world: World) {
        exception_free(|token| {
            let ctx = &mut FGT_CTX.get().borrow_mut(token)[world];

            if read_id_aa64pfr0_el1().is_feat_amu_present() {
                ctx.hafgrtr_el2 = read_hafgrtr_el2();
            }
            ctx.hdfgrtr_el2 = read_hdfgrtr_el2();
            ctx.hdfgwtr_el2 = read_hdfgwtr_el2();
            ctx.hfgitr_el2 = read_hfgitr_el2();
            ctx.hfgrtr_el2 = read_hfgrtr_el2();
            ctx.hfgwtr_el2 = read_hfgwtr_el2();
        })
    }

    pub fn restore_context(world: World) {
        exception_free(|token| {
            let ctx = &FGT_CTX.get().borrow_mut(token)[world];

            if read_id_aa64pfr0_el1().is_feat_amu_present() {
                // SAFETY: We're restoring the value previously saved, so it must be valid.
                unsafe {
                    write_hafgrtr_el2(ctx.hafgrtr_el2);
                }
            }
            // SAFETY: We're restoring the values previously saved, so it must be valid.
            unsafe {
                write_hdfgrtr_el2(ctx.hdfgrtr_el2);
                write_hdfgwtr_el2(ctx.hdfgwtr_el2);
                write_hfgitr_el2(ctx.hfgitr_el2);
                write_hfgrtr_el2(ctx.hfgrtr_el2);
                write_hfgwtr_el2(ctx.hfgwtr_el2);
            }
        })
    }
}

/// FEAT_FGT support
///
/// Enables support for the HFGITR_EL2, HFGRTR_EL2, HFGWTR_EL2, HDFGRTR_EL2, and HDFGWTR_EL2
/// registers, which enable fine-grained traps to EL2 of EL1 and EL0 access to system registers and
/// instructions.
pub struct Fgt;

impl CpuExtension for Fgt {
    fn is_present(&self) -> bool {
        // Assume present as FEAT_FGT is mandatory from Armv8.6.
        true
    }

    fn init(&self) {
        // Write the HFG*_EL2 init values directly to the registers if FGT context switching is
        // disabled.
        // SAFETY: We are initializing system registers with a fixed safe value.
        #[cfg(not(any(feature = "sel2", feature = "rme")))]
        unsafe {
            write_hfgitr_el2(HFGITR_EL2_INIT_VAL);
            write_hfgrtr_el2(HFGRTR_EL2_INIT_VAL);
            write_hfgwtr_el2(HFGWTR_EL2_INIT_VAL);
        }
    }

    #[cfg(any(feature = "sel2", feature = "rme"))]
    #[allow(dead_code)]
    fn save_context(&self, world: World) {
        if self.is_present() {
            fgt_el2::save_context(world);
        }
    }

    #[cfg(any(feature = "sel2", feature = "rme"))]
    fn restore_context(&self, world: World) {
        if self.is_present() {
            fgt_el2::restore_context(world);
        }
    }
}
