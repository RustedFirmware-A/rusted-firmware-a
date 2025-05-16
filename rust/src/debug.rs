// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::logger::build_time_log_level;
use core::arch::global_asm;
use log::LevelFilter;

/// True if the build is configured with debug assertions on.
pub const DEBUG: bool = cfg!(debug_assertions);

#[cfg(target_arch = "aarch64")]
global_asm!(
    include_str!("asm_macros_common.S"),
    include_str!("debug.S"),
    include_str!("asm_macros_common_purge.S"),
    DEBUG = const DEBUG as i32,
    LOG_LEVEL_INFO = const LevelFilter::Info as u32,
    LOG_LEVEL = const build_time_log_level() as u32,
    // TODO: We'll put this back to being `DEBUG as u32` in a subsequent change after the change to
    // move context.S into Rust merges.
    ENABLE_ASSERTIONS = const 1_u32,
);
