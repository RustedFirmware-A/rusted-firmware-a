// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! MPAM context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{
    Mpam2El2, MpamhcrEl2, MpamidrEl1, Mpamvpm0El2, Mpamvpm1El2, Mpamvpm2El2, Mpamvpm3El2,
    Mpamvpm4El2, Mpamvpm5El2, Mpamvpm6El2, Mpamvpm7El2, MpamvpmvEl2, read_mpam2_el2,
    read_mpamhcr_el2, read_mpamidr_el1, read_mpamvpm0_el2, read_mpamvpm1_el2, read_mpamvpm2_el2,
    read_mpamvpm3_el2, read_mpamvpm4_el2, read_mpamvpm5_el2, read_mpamvpm6_el2, read_mpamvpm7_el2,
    read_mpamvpmv_el2, write_mpam2_el2, write_mpamhcr_el2, write_mpamvpm0_el2, write_mpamvpm1_el2,
    write_mpamvpm2_el2, write_mpamvpm3_el2, write_mpamvpm4_el2, write_mpamvpm5_el2,
    write_mpamvpm6_el2, write_mpamvpm7_el2, write_mpamvpmv_el2,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct MpamCpuContext {
    mpam2_el2: Mpam2El2,
    mpamhcr_el2: MpamhcrEl2,
    mpamvpmv_el2: MpamvpmvEl2,
    mpamvpm0_el2: Mpamvpm0El2,
    mpamvpm1_el2: Mpamvpm1El2,
    mpamvpm2_el2: Mpamvpm2El2,
    mpamvpm3_el2: Mpamvpm3El2,
    mpamvpm4_el2: Mpamvpm4El2,
    mpamvpm5_el2: Mpamvpm5El2,
    mpamvpm6_el2: Mpamvpm6El2,
    mpamvpm7_el2: Mpamvpm7El2,
}

impl MpamCpuContext {
    const EMPTY: Self = Self {
        mpam2_el2: Mpam2El2::empty(),
        mpamhcr_el2: MpamhcrEl2::empty(),
        mpamvpmv_el2: MpamvpmvEl2::empty(),
        mpamvpm0_el2: Mpamvpm0El2::empty(),
        mpamvpm1_el2: Mpamvpm1El2::empty(),
        mpamvpm2_el2: Mpamvpm2El2::empty(),
        mpamvpm3_el2: Mpamvpm3El2::empty(),
        mpamvpm4_el2: Mpamvpm4El2::empty(),
        mpamvpm5_el2: Mpamvpm5El2::empty(),
        mpamvpm6_el2: Mpamvpm6El2::empty(),
        mpamvpm7_el2: Mpamvpm7El2::empty(),
    };
}

static MPAM_CTX: PerCoreState<PerWorld<MpamCpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [MpamCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

pub(super) fn save_context(world: World) {
    exception_free(|token| {
        let mut ctx = MPAM_CTX.get().borrow_mut(token);
        ctx[world].mpam2_el2 = read_mpam2_el2();

        let mpamidr_el1 = read_mpamidr_el1();
        if !mpamidr_el1.contains(MpamidrEl1::HAS_HCR) {
            return;
        }

        ctx[world].mpamhcr_el2 = read_mpamhcr_el2();
        ctx[world].mpamvpmv_el2 = read_mpamvpmv_el2();
        ctx[world].mpamvpm0_el2 = read_mpamvpm0_el2();

        let vpmr_max = mpamidr_el1.vpmr_max();
        if vpmr_max == 7 {
            ctx[world].mpamvpm7_el2 = read_mpamvpm7_el2();
        }

        if vpmr_max >= 6 {
            ctx[world].mpamvpm6_el2 = read_mpamvpm6_el2();
        }

        if vpmr_max >= 5 {
            ctx[world].mpamvpm5_el2 = read_mpamvpm5_el2();
        }

        if vpmr_max >= 4 {
            ctx[world].mpamvpm4_el2 = read_mpamvpm4_el2();
        }

        if vpmr_max >= 3 {
            ctx[world].mpamvpm3_el2 = read_mpamvpm3_el2();
        }

        if vpmr_max >= 2 {
            ctx[world].mpamvpm2_el2 = read_mpamvpm2_el2();
        }

        if vpmr_max >= 1 {
            ctx[world].mpamvpm1_el2 = read_mpamvpm1_el2();
        }
    })
}

pub(super) fn restore_context(world: World) {
    exception_free(|token| {
        let ctx = MPAM_CTX.get().borrow_mut(token);

        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_mpam2_el2(ctx[world].mpam2_el2);
        }

        let mpamidr_el1 = read_mpamidr_el1();
        if !mpamidr_el1.contains(MpamidrEl1::HAS_HCR) {
            return;
        }

        // SAFETY: We're restoring the values previously saved, so they must be valid.
        unsafe {
            write_mpamhcr_el2(ctx[world].mpamhcr_el2);
            write_mpamvpmv_el2(ctx[world].mpamvpmv_el2);
            write_mpamvpm0_el2(ctx[world].mpamvpm0_el2);
        }

        let vpmr_max = mpamidr_el1.vpmr_max();
        // SAFETY: We're restoring the values previously saved, so they must be valid.
        unsafe {
            if vpmr_max == 7 {
                write_mpamvpm7_el2(ctx[world].mpamvpm7_el2);
            }
            if vpmr_max >= 6 {
                write_mpamvpm6_el2(ctx[world].mpamvpm6_el2);
            }
            if vpmr_max >= 5 {
                write_mpamvpm5_el2(ctx[world].mpamvpm5_el2);
            }
            if vpmr_max >= 4 {
                write_mpamvpm4_el2(ctx[world].mpamvpm4_el2);
            }
            if vpmr_max >= 3 {
                write_mpamvpm3_el2(ctx[world].mpamvpm3_el2);
            }
            if vpmr_max >= 2 {
                write_mpamvpm2_el2(ctx[world].mpamvpm2_el2);
            }
            if vpmr_max >= 1 {
                write_mpamvpm1_el2(ctx[world].mpamvpm1_el2);
            }
        }
    })
}
