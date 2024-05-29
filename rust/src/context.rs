// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::platform::{Platform, PlatformImpl};
use core::ptr::null_mut;

// TODO: Add support for realm security state.
/// The number of contexts to store for each CPU core, one per security state.
const CPU_DATA_CONTEXT_NUM: usize = 2;

/// The maximum number of runtime services that we can support.
const MAX_RT_SVCS: usize = 128;

const CPU_DATA_CRASH_BUF_SIZE: usize = 64;

/// The state of a core at the next lower EL in a given security state.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CpuContext {
    gpregs: GpRegs,
    el3_state: El3State,
    el1_sysregs: El1Sysregs,
}

/// AArch64 general purpose register context structure. Usually x0-x18 and lr are saved as the
/// compiler is expected to preserve the remaining callee saved registers if needed and the assembly
/// code does not touch the remaining. But in case of world switch during exception handling,
/// we need to save the callee registers too.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
struct GpRegs {
    registers: [u64; Self::COUNT],
}

impl GpRegs {
    /// The number of (64-bit) registers included in `GpRegs`.
    const COUNT: usize = 32;
}

/// Miscellaneous registers used by EL3 firmware to maintain its state across exception entries and
/// exits.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
struct El3State {
    registers: [u64; Self::COUNT],
}

impl El3State {
    /// The number of (64-bit) registers included in `El3State`.
    const COUNT: usize = 10;
}

/// AArch64 EL1 system register context structure for preserving the architectural state during
/// world switches.
#[derive(Clone, Debug)]
#[repr(C, align(16))]
struct El1Sysregs {
    registers: [u64; Self::COUNT],
}

impl El1Sysregs {
    /// The number of (64-bit) registers included in `EL1Sysregs`.
    const COUNT: usize = 28;
}

/// Registers whose values can be shared across CPUs.
#[derive(Clone, Debug, Default)]
#[repr(C)]
struct PerWorldContext {
    cptr_el3: u64,
    zcr_el3: u64,
}

#[derive(Clone, Debug)]
#[repr(C, align(64))]
struct CpuData {
    cpu_context: [*mut u8; CPU_DATA_CONTEXT_NUM],
    cpu_ops_ptr: usize,
    psci_svc_cpu_data: PsciCpuData,
    crash_buf: [u64; CPU_DATA_CRASH_BUF_SIZE >> 3],
}

impl CpuData {
    const EMPTY: Self = Self {
        cpu_context: [null_mut(); CPU_DATA_CONTEXT_NUM],
        cpu_ops_ptr: 0,
        psci_svc_cpu_data: PsciCpuData {
            aff_info_state: AffInfoState::On,
            target_pwrlvl: 0,
            local_state: 0,
        },
        crash_buf: [0; CPU_DATA_CRASH_BUF_SIZE >> 3],
    };
}

#[derive(Clone, Debug)]
#[repr(C)]
struct PsciCpuData {
    aff_info_state: AffInfoState,
    target_pwrlvl: u32,
    local_state: u8,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(C)]
enum AffInfoState {
    On = 0,
    Off = 1,
    OnPending = 2,
}

#[export_name = "per_world_context"]
static mut PER_WORLD_CONTEXT: [PerWorldContext; CPU_DATA_CONTEXT_NUM] = [
    PerWorldContext {
        cptr_el3: 0,
        zcr_el3: 0,
    },
    PerWorldContext {
        cptr_el3: 0,
        zcr_el3: 0,
    },
];

#[export_name = "rt_svc_descs_indices"]
static mut RT_SVC_DESCS_INDICES: [u8; MAX_RT_SVCS] = [0xff; MAX_RT_SVCS];

#[no_mangle]
static mut percpu_data: [CpuData; PlatformImpl::CORE_COUNT] =
    [CpuData::EMPTY; PlatformImpl::CORE_COUNT];
