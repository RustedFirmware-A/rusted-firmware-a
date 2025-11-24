// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_TCR2 context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{Tcr2El2, read_tcr2_el2, write_tcr2_el2};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct Tcr2CpuContext {
    tcr2_el2: Tcr2El2,
}

impl Tcr2CpuContext {
    const EMPTY: Self = Self {
        tcr2_el2: Tcr2El2::empty(),
    };
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
        let mut ctx = TCR2_CTX.get().borrow_mut(token);
        ctx[world].tcr2_el2 = read_tcr2_el2();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        let ctx = TCR2_CTX.get().borrow_mut(token);
        // SAFETY: We're restoring the value previously saved, so it must be valid.
        unsafe {
            write_tcr2_el2(ctx[world].tcr2_el2);
        }
    })
}
