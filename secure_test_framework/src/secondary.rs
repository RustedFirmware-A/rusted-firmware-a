// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Stacks and entry point for secondary cores.

use crate::{
    platform::{Platform, PlatformImpl},
    secondary_main,
};
use aarch64_rt::Stack;
use core::arch::naked_asm;

/// The number of 4 KiB pages to reserve for each secondary CPU stack.
const SECONDARY_STACK_PAGES: usize = 1 << SECONDARY_STACK_PAGES_LOG2;

/// log2 of the desired value for `SECONDARY_STACK_PAGES`.
const SECONDARY_STACK_PAGES_LOG2: usize = 1;

/// Stacks for secondary cores. The primary core is special, its stack is reserved by `aarch64-rt`.
static mut SECONDARY_STACKS: [Stack<SECONDARY_STACK_PAGES>; PlatformImpl::CORE_COUNT - 1] =
    [const { Stack::new() }; PlatformImpl::CORE_COUNT - 1];

#[unsafe(naked)]
pub unsafe extern "C" fn secondary_entry() -> ! {
    naked_asm!(
        // Disable trapping floating point access in EL1.
        "mrs x30, cpacr_el1",
        "orr x30, x30, #(0x3 << 20)",
        "msr cpacr_el1, x30",
        "isb",

        // Save registers which `core_position` might clobber.
        "mov x24, x0",
        "mov x25, x1",
        "mov x26, x2",
        "mov x27, x3",

        // Find the current CPU's linear index.
        "mrs x0, mpidr_el1",
        "bl {core_position}",

        // TODO: Make sure that x0 isn't 0.
        // Get the stack for the current CPU. CPU n should get SECONDARY_STACKS[n - 1], but stacks
        // grow down so the offset cancels out.
        "adrp x30, {SECONDARY_STACKS}",
        "add x30, x30, :lo12:{SECONDARY_STACKS}",
        "add x30, x30, x0, lsr #{SECONDARY_STACK_SHIFT}",
        "mov sp, x30",

        // Restore registers x0-x3.
        "mov x0, x24",
        "mov x1, x25",
        "mov x2, x26",
        "mov x3, x27",

        // Call into Rust code.
        "b {secondary_main}",
        core_position = sym PlatformImpl::core_position,
        SECONDARY_STACKS = sym SECONDARY_STACKS,
        SECONDARY_STACK_SHIFT = const SECONDARY_STACK_PAGES_LOG2 + 12,
        secondary_main = sym secondary_main,
    );
}
