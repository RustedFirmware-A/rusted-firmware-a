// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_RAS context management for when Secure EL2 is enabled.

use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, World},
    platform::{Platform, PlatformImpl, exception_free},
};
use arm_sysregs::{
    VdisrEl2, VsesrEl2, read_vdisr_el2, read_vsesr_el2, write_vdisr_el2, write_vsesr_el2,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

struct RasCpuContext {
    vdisr_el2: VdisrEl2,
    vsesr_el2: VsesrEl2,
}

impl RasCpuContext {
    const EMPTY: Self = Self {
        vdisr_el2: VdisrEl2::empty(),
        vsesr_el2: VsesrEl2::empty(),
    };
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
        let mut ctx = RAS_CTX.get().borrow_mut(token);
        ctx[world].vdisr_el2 = read_vdisr_el2();
        ctx[world].vsesr_el2 = read_vsesr_el2();
    })
}

pub fn restore_context(world: World) {
    exception_free(|token| {
        let ctx = RAS_CTX.get().borrow_mut(token);
        write_vdisr_el2(ctx[world].vdisr_el2);
        write_vsesr_el2(ctx[world].vsesr_el2);
    })
}
