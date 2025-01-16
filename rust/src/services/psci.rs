// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::World,
    platform::{Platform, PlatformImpl},
    services::{arch::SMCCC_VERSION, owns, Service},
    smccc::{FunctionId, OwningEntityNumber, SmcReturn, NOT_SUPPORTED},
};

const PSCI_VERSION: u32 = 0x8400_0000;
#[allow(unused)]
const PSCI_CPU_SUSPEND_32: u32 = 0x8400_0001;
#[allow(unused)]
const PSCI_CPU_SUSPEND_64: u32 = 0xC400_0001;
#[allow(unused)]
const PSCI_CPU_OFF: u32 = 0x8400_0002;
#[allow(unused)]
const PSCI_CPU_ON_32: u32 = 0x8400_0003;
#[allow(unused)]
const PSCI_CPU_ON_64: u32 = 0xC400_0003;
#[allow(unused)]
const PSCI_AFFINITY_INFO_32: u32 = 0x8400_0004;
#[allow(unused)]
const PSCI_AFFINITY_INFO_64: u32 = 0xC400_0004;
#[allow(unused)]
const PSCI_MIGRATE_32: u32 = 0x8400_0005;
#[allow(unused)]
const PSCI_MIGRATE_64: u32 = 0xC400_0005;
#[allow(unused)]
const PSCI_MIGRATE_INFO_TYPE: u32 = 0x8400_0006;
#[allow(unused)]
const PSCI_MIGRATE_INFO_UP_CPU_32: u32 = 0x8400_0007;
#[allow(unused)]
const PSCI_MIGRATE_INFO_UP_CPU_64: u32 = 0xC400_0007;
const PSCI_SYSTEM_OFF: u32 = 0x8400_0008;
#[allow(unused)]
const PSCI_SYSTEM_RESET: u32 = 0x8400_0009;
#[allow(unused)]
const PSCI_SYSTEM_RESET2_32: u32 = 0x8400_0012;
#[allow(unused)]
const PSCI_SYSTEM_RESET2_64: u32 = 0xC400_0012;
#[allow(unused)]
const PSCI_MEM_PROTECT: u32 = 0x8400_0013;
#[allow(unused)]
const PSCI_MEM_PROTECT_CHECK_RANGE_32: u32 = 0x8400_0014;
#[allow(unused)]
const PSCI_MEM_PROTECT_CHECK_RANGE_64: u32 = 0xC400_0014;
const PSCI_FEATURES: u32 = 0x8400_000A;
#[allow(unused)]
const PSCI_CPU_FREEZE: u32 = 0x8400_000B;
#[allow(unused)]
const PSCI_CPU_DEFAULT_SUSPEND_32: u32 = 0x8400_000C;
#[allow(unused)]
const PSCI_CPU_DEFAULT_SUSPEND_64: u32 = 0xC400_000C;
#[allow(unused)]
const PSCI_NODE_HW_STATE_32: u32 = 0x8400_000D;
#[allow(unused)]
const PSCI_NODE_HW_STATE_64: u32 = 0xC400_000D;
#[allow(unused)]
const PSCI_SYSTEM_SUSPEND_32: u32 = 0x8400_000E;
#[allow(unused)]
const PSCI_SYSTEM_SUSPEND_64: u32 = 0xC400_000E;
#[allow(unused)]
const PSCI_SET_SUSPEND_MODE: u32 = 0x8400_000F;
#[allow(unused)]
const PSCI_STAT_RESIDENCY_32: u32 = 0x8400_0010;
#[allow(unused)]
const PSCI_STAT_RESIDENCY_64: u32 = 0xC400_0010;
#[allow(unused)]
const PSCI_STAT_COUNT_32: u32 = 0x8400_0011;
#[allow(unused)]
const PSCI_STAT_COUNT_64: u32 = 0xC400_0011;

const PSCI_VERSION_1_1: u32 = 0x0001_0001;

const FUNCTION_NUMBER_MIN: u16 = 0x0000;
const FUNCTION_NUMBER_MAX: u16 = 0x001F;

/// Power State Coordination Interface.
pub struct Psci;

impl Service for Psci {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_smc(
        function: FunctionId,
        x1: u64,
        _x2: u64,
        _x3: u64,
        _x4: u64,
        _world: World,
    ) -> SmcReturn {
        match function.0 {
            PSCI_VERSION => version().into(),
            PSCI_SYSTEM_OFF => system_off(),
            PSCI_FEATURES => psci_features(x1 as u32).into(),
            _ => NOT_SUPPORTED.into(),
        }
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
        SMCCC_VERSION | PSCI_VERSION | PSCI_SYSTEM_OFF | PSCI_FEATURES => 0,
        _ => NOT_SUPPORTED,
    }
}
