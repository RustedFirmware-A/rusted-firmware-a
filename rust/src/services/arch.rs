// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    exceptions::SmcFlags,
    services::owns,
    smccc::{
        FunctionId, OwningEntity, OwningEntityNumber, SmcReturn, SmcccCallType, INVALID_PARAMETER,
        NOT_SUPPORTED, SUCCESS,
    },
};

pub const SMCCC_VERSION: u32 = 0x8000_0000;
const SMCCC_ARCH_FEATURES: u32 = 0x8000_0001;
const SMCCC_ARCH_SOC_ID_32: u32 = 0x8000_0002;
const SMCCC_ARCH_SOC_ID_64: u32 = 0xc000_0002;
const SMCCC_ARCH_SOC_ID_VERSION: u32 = 0x0;
const SMCCC_ARCH_SOC_ID_REVISION: u32 = 0x1;
const SMCCC_ARCH_SOC_ID_NAME: u32 = 0x2;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_1: u32 = 0x8000_8000;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_2: u32 = 0x8000_7FFF;
#[allow(unused)]
const SMCCC_ARCH_WORKAROUND_3: u32 = 0x8000_3FFF;

pub const SMCCC_VERSION_1_5: i32 = 0x0001_0005;

// Defines the range of SMC function ID values covered by the arch service
owns! {OwningEntity::ArmArchitectureService}

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
        SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => arch_soc_id(x1 as u32, function.call_type()),
        _ => NOT_SUPPORTED.into(),
    }
}

fn version() -> i32 {
    SMCCC_VERSION_1_5
}

fn arch_features(arch_func_id: u32) -> i32 {
    match arch_func_id {
        SMCCC_VERSION | SMCCC_ARCH_FEATURES | SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => {
            SUCCESS
        }
        _ => NOT_SUPPORTED,
    }
}

// This SMC is specified in §7.4 of [the Arm SMC Calling
// Convention](https://developer.arm.com/documentation/den0028/galp1/?lang=en).
fn arch_soc_id(soc_id_type: u32, call_type: SmcccCallType) -> SmcReturn {
    // TODO/NOTE: Note that according to the SMCCC spec, section 7.4.6: we "must
    // ensure that SoC version and revision uniquely identify the SoC", and "SoC
    // name must not contain SoC identifying information not captured by <SoC
    // version, SoC revision>."
    match soc_id_type {
        SMCCC_ARCH_SOC_ID_VERSION => 0.into(), // TODO: Implement this properly.
        SMCCC_ARCH_SOC_ID_REVISION => 0.into(), // TODO: Implement this properly.
        SMCCC_ARCH_SOC_ID_NAME if call_type == SmcccCallType::Fast64 => {
            [
                // TODO: Implement this properly.
                0u64, // w0
                u64::from_le_bytes([b'm', b'I', b' ', b':', b'O', b'D', b'O', b'T']),
                u64::from_le_bytes([b' ', b't', b'n', b'e', b'm', b'e', b'l', b'p']),
                u64::from_le_bytes([b'o', b'r', b'p', b' ', b's', b'i', b'h', b't']),
                u64::from_le_bytes([0x00, 0x00, b'.', b'y', b'l', b'r', b'e', b'p']),
            ]
            .into()
        }
        _ => INVALID_PARAMETER.into(),
    }
}
