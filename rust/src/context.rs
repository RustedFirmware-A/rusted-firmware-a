// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    platform::{Platform, PlatformImpl},
    sysregs::write_sctlr_el1,
};
use core::{
    arch::asm,
    ptr::{addr_of_mut, null_mut},
};

// TODO: Add support for realm security state.
/// The number of contexts to store for each CPU core, one per security state.
const CPU_DATA_CONTEXT_NUM: usize = 2;

/// The maximum number of runtime services that we can support.
const MAX_RT_SVCS: usize = 128;

const CPU_DATA_CRASH_BUF_SIZE: usize = 64;

// Indices of registers within `El3State.registers`.
const CTX_SCR_EL3: usize = 0;
const CTX_SPSR_EL3: usize = 3;
const CTX_ELR_EL3: usize = 4;

// Indices of registers within `El1Sysregs.registers`.
const CTX_SCTLR_EL1: usize = 2;

/// RES1 bits in the `scr_el3` register.
const SCR_RES1: u64 = 1 << 4 | 1 << 5;
const SCR_NS: u64 = 1 << 0;
const SCR_RW: u64 = 1 << 10;

/// RES1 bits in the `sctlr_el1` register.
const SCTLR_EL1_RES1: u64 = 1 << 29 | 1 << 28 | 1 << 23 | 1 << 22 | 1 << 20 | 1 << 11;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum World {
    NonSecure = 0,
    Secure = 1,
}

impl World {
    fn index(self) -> usize {
        self as usize
    }
}

/// The state of a core at the next lower EL in a given security state.
#[derive(Clone, Debug)]
#[repr(C)]
pub struct CpuContext {
    gpregs: GpRegs,
    el3_state: El3State,
    el1_sysregs: El1Sysregs,
}

impl CpuContext {
    const EMPTY: Self = Self {
        gpregs: GpRegs::EMPTY,
        el3_state: El3State::EMPTY,
        el1_sysregs: El1Sysregs::EMPTY,
    };
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

    const EMPTY: Self = Self {
        registers: [0; Self::COUNT],
    };
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

    const EMPTY: Self = Self {
        registers: [0; Self::COUNT],
    };
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

    const EMPTY: Self = Self {
        registers: [0; Self::COUNT],
    };
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

static mut CPU_CONTEXTS: [CpuContext; CPU_DATA_CONTEXT_NUM] =
    [CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM];

/// Sets SP_EL3 to a pointer to the given CpuContext, ready for exception return.
///
/// # Safety
///
/// The given context pointer must remain valid until a new next context is set.
unsafe fn set_next_context(context: *mut CpuContext) {
    unsafe {
        asm!(
            "msr spsel, #1",
            "mov sp, {context}",
            "msr spsel, #0",
            context = in(reg) context,
        )
    }
}

/// Selects the given world to run on the next exception return.
///
/// This works by setting `SP_EL3` to point to the appropriate `CpuContext` struct, so the
/// exception return code will restore registers from it before the `eret`.
pub fn set_next_world_context(world: World) {
    unsafe { set_next_context(addr_of_mut!(CPU_CONTEXTS[world.index()])) }
}

/// Initialises all CPU contexts ready for first boot.
pub fn initialise_contexts(non_secure_entry_point: &EntryPointInfo) {
    unsafe {
        initialise_nonsecure(
            &mut CPU_CONTEXTS[World::NonSecure.index()],
            non_secure_entry_point,
        );
    }
}

/// Initialises the given CPU context ready for booting NS-EL2 or NS-EL1.
fn initialise_nonsecure(context: &mut CpuContext, entry_point: &EntryPointInfo) {
    context.el3_state.registers[CTX_ELR_EL3] = entry_point.pc;
    // TODO: FIQ and IRQ routing model.
    let scr_el3 = SCR_RES1 | SCR_NS | SCR_RW;
    context.el3_state.registers[CTX_SCR_EL3] = scr_el3;
    context.el3_state.registers[CTX_SPSR_EL3] = entry_point.spsr;
    // TODO: Initialise EL2 state too.
    let sctlr_el1 = SCTLR_EL1_RES1;
    context.el1_sysregs.registers[CTX_SCTLR_EL1] = sctlr_el1;
    write_sctlr_el1(sctlr_el1);
}

/// Information about the entry point for a next stage (e.g. BL32 or BL33).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EntryPointInfo {
    /// The entry point address.
    pub pc: u64,
    /// The `spsr_el3` value to set before `eret`, to set the appropriate PSTATE.
    pub spsr: u64,
    /// Boot arguments to pass in `x0`-`x7`.
    pub args: [u64; 8],
}
