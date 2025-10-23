// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_HCX context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{HcrxEl2, read_hcrx_el2, write_hcrx_el2};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct HcxCpuContext {
    hcrx_el2: HcrxEl2,
}

impl HcxCpuContext {
    const EMPTY: Self = Self {
        hcrx_el2: HcrxEl2::empty(),
    };
}

static HCX_CTX: PerCoreState<PerWorld<HcxCpuContext>> = PerCore::new(
    [const {
        ExceptionLock::new(RefCell::new(PerWorld(
            [HcxCpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
        )))
    }; PlatformImpl::CORE_COUNT],
);

pub fn save_context(world: World) {
    exception_free(|token| {
        HCX_CTX.get().borrow_mut(token)[world].hcrx_el2 = read_hcrx_el2();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        write_hcrx_el2(HCX_CTX.get().borrow_mut(token)[world].hcrx_el2);
    })
}
