// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_MTE2 context management for when Secure EL2 is not enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{
    read_gcr_el1, read_rgsr_el1, read_tfsr_el1, read_tfsre0_el1, write_gcr_el1, write_rgsr_el1,
    write_tfsr_el1, write_tfsre0_el1,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

static MTE2_CTX: PerCoreState<PerWorld<Mte2CpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [Mte2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

struct Mte2CpuContext {
    tfsre0_el1: u64,
    tfsr_el1: u64,
    rgsr_el1: u64,
    gcr_el1: u64,
}

impl Mte2CpuContext {
    const EMPTY: Self = Self {
        tfsre0_el1: 0,
        tfsr_el1: 0,
        rgsr_el1: 0,
        gcr_el1: 0,
    };
}

pub fn save_context(world: World) {
    exception_free(|token| {
        let mut ctx = MTE2_CTX.get().borrow_mut(token);
        ctx[world].tfsre0_el1 = read_tfsre0_el1();
        ctx[world].tfsr_el1 = read_tfsr_el1();
        ctx[world].rgsr_el1 = read_rgsr_el1();
        ctx[world].gcr_el1 = read_gcr_el1();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        let ctx = MTE2_CTX.get().borrow_mut(token);
        write_tfsre0_el1(ctx[world].tfsre0_el1);
        write_tfsr_el1(ctx[world].tfsr_el1);
        write_rgsr_el1(ctx[world].rgsr_el1);
        write_gcr_el1(ctx[world].gcr_el1);
    })
}
