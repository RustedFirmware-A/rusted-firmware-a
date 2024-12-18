// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    exceptions::SmcFlags,
    platform::{Platform, PlatformImpl},
    services::arch::SMCCC_VERSION,
    services::owns,
    smccc::{
        FunctionId, OwningEntity, OwningEntityNumber, SmcReturn, SmcccCallType, NOT_SUPPORTED,
        SUCCESS,
    },
};

const PSCI_VERSION: u32 = 0x84000000;
#[allow(unused)]
const PSCI_CPU_SUSPEND_32: u32 = 0x84000001;
#[allow(unused)]
const PSCI_CPU_SUSPEND_64: u32 = 0xC4000001;
#[allow(unused)]
const PSCI_CPU_OFF: u32 = 0x84000002;
#[allow(unused)]
const PSCI_CPU_ON_32: u32 = 0x84000003;
#[allow(unused)]
const PSCI_CPU_ON_64: u32 = 0xC4000003;
#[allow(unused)]
const PSCI_AFFINITY_INFO_32: u32 = 0x84000004;
#[allow(unused)]
const PSCI_AFFINITY_INFO_64: u32 = 0xC4000004;
#[allow(unused)]
const PSCI_MIGRATE_32: u32 = 0x84000005;
#[allow(unused)]
const PSCI_MIGRATE_64: u32 = 0xC4000005;
#[allow(unused)]
const PSCI_MIGRATE_INFO_TYPE: u32 = 0x84000006;
#[allow(unused)]
const PSCI_MIGRATE_INFO_UP_CPU_32: u32 = 0x84000007;
#[allow(unused)]
const PSCI_MIGRATE_INFO_UP_CPU_64: u32 = 0xC4000007;
const PSCI_SYSTEM_OFF: u32 = 0x84000008;
#[allow(unused)]
const PSCI_SYSTEM_RESET: u32 = 0x84000009;
#[allow(unused)]
const PSCI_SYSTEM_RESET2_32: u32 = 0x84000012;
#[allow(unused)]
const PSCI_SYSTEM_RESET2_64: u32 = 0xC4000012;
#[allow(unused)]
const PSCI_MEM_PROTECT: u32 = 0x84000013;
#[allow(unused)]
const PSCI_MEM_PROTECT_CHECK_RANGE_32: u32 = 0x84000014;
#[allow(unused)]
const PSCI_MEM_PROTECT_CHECK_RANGE_64: u32 = 0xC4000014;
const PSCI_FEATURES: u32 = 0x8400000A;
#[allow(unused)]
const PSCI_CPU_FREEZE: u32 = 0x8400000B;
#[allow(unused)]
const PSCI_CPU_DEFAULT_SUSPEND_32: u32 = 0x8400000C;
#[allow(unused)]
const PSCI_CPU_DEFAULT_SUSPEND_64: u32 = 0xC400000C;
#[allow(unused)]
const PSCI_NODE_HW_STATE_32: u32 = 0x8400000D;
#[allow(unused)]
const PSCI_NODE_HW_STATE_64: u32 = 0xC400000D;
#[allow(unused)]
const PSCI_SYSTEM_SUSPEND_32: u32 = 0x8400000E;
#[allow(unused)]
const PSCI_SYSTEM_SUSPEND_64: u32 = 0xC400000E;
#[allow(unused)]
const PSCI_SET_SUSPEND_MODE: u32 = 0x8400000F;
#[allow(unused)]
const PSCI_STAT_RESIDENCY_32: u32 = 0x84000010;
#[allow(unused)]
const PSCI_STAT_RESIDENCY_64: u32 = 0xC4000010;
#[allow(unused)]
const PSCI_STAT_COUNT_32: u32 = 0x84000011;
#[allow(unused)]
const PSCI_STAT_COUNT_64: u32 = 0xC4000011;

const PSCI_VERSION_1_1: u32 = 0x0001_0001;

// Defines the range of SMC function ID values covered by the psci.rs service
owns! {OwningEntity::StandardSecureService, RangeInclusive::new(0x0000, 0x001F)}

/// Handles a PSCI SMC.
pub fn handle_smc(
    function: FunctionId,
    x1: u64,
    _x2: u64,
    _x3: u64,
    _x4: u64,
    _flags: SmcFlags,
) -> SmcReturn {
    match function.0 {
        PSCI_VERSION => version().into(),
        PSCI_SYSTEM_OFF => system_off(),
        PSCI_FEATURES => psci_features(x1 as u32).into(),
        _ => NOT_SUPPORTED.into(),
    }
}

fn version() -> u32 {
    PSCI_VERSION_1_1
}

fn system_off() -> ! {
    // TODO: Notify SPD, flush console.

    PlatformImpl::system_off()
}

fn psci_features(function_id: u32) -> i32 {
    match function_id {
        SMCCC_VERSION | PSCI_VERSION | PSCI_SYSTEM_OFF | PSCI_FEATURES => SUCCESS,
        _ => NOT_SUPPORTED,
    }
}
