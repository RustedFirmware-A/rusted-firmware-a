// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_RAS context management for when Secure EL2 is not enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{read_disr_el1, write_disr_el1};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct RasCpuContext {
    disr_el1: u64,
}

impl RasCpuContext {
    const EMPTY: Self = Self { disr_el1: 0 };
}

static RAS_CTX: PerCoreState<PerWorld<RasCpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [RasCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

pub fn save_context(world: World) {
    exception_free(|token| {
        RAS_CTX.get().borrow_mut(token)[world].disr_el1 = read_disr_el1();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        write_disr_el1(RAS_CTX.get().borrow_mut(token)[world].disr_el1);
    })
}
