// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    debug::DEBUG,
    platform::{Platform, PlatformImpl},
};
use core::arch::global_asm;

/// The number of bytes of stack space to reserve for each core.
const STACK_SIZE: usize = 0x2000;

global_asm!(
    include_str!("asm_macros_common.S"),

    // Helper assembler macro to count trailing zeros. The output is populated in the `TZ_COUNT`
    // symbol.
    ".macro count_tz _value, _tz_count",
    ".if \\_value",
    "  count_tz \"(\\_value >> 1)\", \"(\\_tz_count + 1)\"",
    ".else",
    "  .equ TZ_COUNT, (\\_tz_count - 1)",
    ".endif",
    ".endm",

    // Returns a pointer to the top of the stack to use for current CPU.
    ".weak	plat_get_my_stack",
        "func plat_get_my_stack",
        "mov	x10, x30",
        "bl	plat_my_core_pos",
        "adrp	x2, (platform_normal_stacks + {STACK_SIZE})",
        "add	x2, x2, :lo12:(platform_normal_stacks + {STACK_SIZE})",
        "mov x1, #{STACK_SIZE}",
        "madd x0, x0, x1, x2",
        "ret	x10",
    "endfunc plat_get_my_stack",

    // Initialises the stack pointer for the current CPU.
    ".weak	plat_set_my_stack",
    "func plat_set_my_stack",
        "mov	x9, x30",
        "bl 	plat_get_my_stack",
        "mov	sp, x0",
        "ret	x9",
    "endfunc plat_set_my_stack",

    "count_tz {CACHE_WRITEBACK_GRANULE}, 0",
    ".if ({CACHE_WRITEBACK_GRANULE} - (1 << TZ_COUNT))",
    "  .error \"Incorrect stack alignment specified (Must be a power of 2).\"",
    ".endif",
    ".if (({STACK_SIZE} & ((1 << TZ_COUNT) - 1)) <> 0)",
    "  .error \"Stack size not correctly aligned\"",
    ".endif",
    ".section    .tzfw_normal_stacks, \"aw\", %nobits",
    ".align TZ_COUNT",
    "platform_normal_stacks:",
    ".space (({PLATFORM_CORE_COUNT}) * ({STACK_SIZE})), 0",
    include_str!("asm_macros_common_purge.S"),

    DEBUG = const DEBUG as i32,
    STACK_SIZE = const STACK_SIZE,
    PLATFORM_CORE_COUNT = const PlatformImpl::CORE_COUNT,
    CACHE_WRITEBACK_GRANULE = const PlatformImpl::CACHE_WRITEBACK_GRANULE,
);
