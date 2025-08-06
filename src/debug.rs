// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// True if the build is configured with debug assertions on.
pub const DEBUG: bool = cfg!(debug_assertions);

// TODO: Make this equal to `DEBUG` once we stop building assembly files from build.rs with a different value.
#[cfg(not(test))]
pub const ENABLE_ASSERTIONS: bool = true;

// TODO: Make this configurable or equal to `DEBUG` once we stop building assembly files from
// build.rs with a different value.
#[cfg(not(test))]
pub const CRASH_REPORTING: bool = true;

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use super::*;
    use crate::logger::build_time_log_level;
    use core::arch::global_asm;
    use log::LevelFilter;

    global_asm!(
        include_str!("asm_macros_common.S"),
        include_str!("debug.S"),
        include_str!("asm_macros_common_purge.S"),
        DEBUG = const DEBUG as i32,
        LOG_LEVEL_INFO = const LevelFilter::Info as u32,
        LOG_LEVEL = const build_time_log_level() as u32,
        ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
    );
}
