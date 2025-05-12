// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::arch::global_asm;

/// True if the build is configured with debug assertions on.
pub const DEBUG: bool = cfg!(debug_assertions);

// const LOG_LEVEL_NONE: u8 = 0;
// const LOG_LEVEL_ERROR: u8 = 10;
// const LOG_LEVEL_NOTICE: u8 = 20;
// const LOG_LEVEL_WARNING: u8 = 30;
const LOG_LEVEL_INFO: u8 = 40;
// const LOG_LEVEL_VERBOSE: u8 = 50;

#[cfg(target_arch = "aarch64")]
global_asm!(
    include_str!("asm_macros_common.S"),
    include_str!("debug.S"),
    include_str!("asm_macros_common_purge.S"),
    DEBUG = const DEBUG as i32,
    LOG_LEVEL_INFO = const LOG_LEVEL_INFO,
    LOG_LEVEL = const LOG_LEVEL_INFO,
    ENABLE_ASSERTIONS = const 1_u8,
);
