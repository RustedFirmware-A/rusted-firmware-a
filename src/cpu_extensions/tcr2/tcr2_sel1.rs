// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 context management for when Secure EL2 is not enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{read_tcr2_el1, write_tcr2_el1};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct Tcr2CpuContext {
    tcr2_el1: u64,
}

impl Tcr2CpuContext {
    const EMPTY: Self = Self { tcr2_el1: 0 };
}

static TCR2_CTX: PerCoreState<PerWorld<Tcr2CpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [Tcr2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

pub fn save_context(world: World) {
    exception_free(|token| {
        TCR2_CTX.get().borrow_mut(token)[world].tcr2_el1 = read_tcr2_el1();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        write_tcr2_el1(TCR2_CTX.get().borrow_mut(token)[world].tcr2_el1);
    })
}
