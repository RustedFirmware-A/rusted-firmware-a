// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_MTE2 context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{TfsrEl2, read_tfsr_el2, write_tfsr_el2};
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
    tfsr_el2: TfsrEl2,
}

impl Mte2CpuContext {
    const EMPTY: Self = Self {
        tfsr_el2: TfsrEl2::empty(),
    };
}

pub fn save_context(world: World) {
    exception_free(|token| {
        let mut ctx = MTE2_CTX.get().borrow_mut(token);
        ctx[world].tfsr_el2 = read_tfsr_el2();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        let ctx = MTE2_CTX.get().borrow_mut(token);
        write_tfsr_el2(ctx[world].tfsr_el2);
    })
}
