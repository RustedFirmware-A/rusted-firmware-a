// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    exceptions::SmcFlags,
    smccc::{FunctionId, SmcReturn, NOT_SUPPORTED, SUCCESS},
};

pub const OEN: u8 = 0;

const SMCCC_VERSION: u32 = 0x8000_0000;
const SMCCC_ARCH_FEATURES: u32 = 0x8000_0001;
#[allow(unused)]
const SMCCC_ARCH_SOC_ID: u32 = 0x8000_0002;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_1: u32 = 0x8000_8000;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_2: u32 = 0x8000_7FFF;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_3: u32 = 0x8000_3FFF;

const SMCCC_VERSION_1_5: i32 = 0x0001_0005;

/// Handles an Arm architecture SMC.
pub fn handle_smc(
    function: FunctionId,
    x1: u64,
    _x2: u64,
    _x3: u64,
    _x4: u64,
    _flags: SmcFlags,
) -> SmcReturn {
    match function.0 {
        SMCCC_VERSION => version().into(),
        SMCCC_ARCH_FEATURES => arch_features(x1 as u32).into(),
        _ => NOT_SUPPORTED.into(),
    }
}

fn version() -> i32 {
    SMCCC_VERSION_1_5
}

fn arch_features(arch_func_id: u32) -> i32 {
    match arch_func_id {
        SMCCC_VERSION | SMCCC_ARCH_FEATURES => SUCCESS,
        _ => NOT_SUPPORTED,
    }
}
