// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! MPAM context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{
    MpamIdrEl1, read_mpam2_el2, read_mpamhcr_el2, read_mpamidr_el1, read_mpamvpm0_el2,
    read_mpamvpm1_el2, read_mpamvpm2_el2, read_mpamvpm3_el2, read_mpamvpm4_el2, read_mpamvpm5_el2,
    read_mpamvpm6_el2, read_mpamvpm7_el2, read_mpamvpmv_el2, write_mpam2_el2, write_mpamhcr_el2,
    write_mpamvpm0_el2, write_mpamvpm1_el2, write_mpamvpm2_el2, write_mpamvpm3_el2,
    write_mpamvpm4_el2, write_mpamvpm5_el2, write_mpamvpm6_el2, write_mpamvpm7_el2,
    write_mpamvpmv_el2,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct MpamCpuContext {
    mpam2_el2: u64,
    mpamhcr_el2: u64,
    mpamvpmv_el2: u64,
    mpamvpm0_el2: u64,
    mpamvpm1_el2: u64,
    mpamvpm2_el2: u64,
    mpamvpm3_el2: u64,
    mpamvpm4_el2: u64,
    mpamvpm5_el2: u64,
    mpamvpm6_el2: u64,
    mpamvpm7_el2: u64,
}

impl MpamCpuContext {
    const EMPTY: Self = Self {
        mpam2_el2: 0,
        mpamhcr_el2: 0,
        mpamvpmv_el2: 0,
        mpamvpm0_el2: 0,
        mpamvpm1_el2: 0,
        mpamvpm2_el2: 0,
        mpamvpm3_el2: 0,
        mpamvpm4_el2: 0,
        mpamvpm5_el2: 0,
        mpamvpm6_el2: 0,
        mpamvpm7_el2: 0,
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
        if !mpamidr_el1.contains(MpamIdrEl1::HAS_HCR) {
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

        write_mpam2_el2(ctx[world].mpam2_el2);

        let mpamidr_el1 = read_mpamidr_el1();
        if !mpamidr_el1.contains(MpamIdrEl1::HAS_HCR) {
            return;
        }

        write_mpamhcr_el2(ctx[world].mpamhcr_el2);
        write_mpamvpmv_el2(ctx[world].mpamvpmv_el2);
        write_mpamvpm0_el2(ctx[world].mpamvpm0_el2);

        let vpmr_max = mpamidr_el1.vpmr_max();
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
    })
}
