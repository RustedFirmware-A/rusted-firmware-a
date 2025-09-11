// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// True if the build is configured with debug assertions on.
pub const DEBUG: bool = cfg!(debug_assertions);

#[cfg(not(test))]
pub const ENABLE_ASSERTIONS: bool = DEBUG;

// TODO: Should this be configurable separately from `DEBUG`?
#[cfg(not(test))]
pub const CRASH_REPORTING: bool = DEBUG;

#[cfg(all(target_arch = "aarch64", not(test)))]
mod asm {
    use super::*;
    use crate::{
        logger::build_time_log_level,
        platform::{Platform, PlatformImpl},
    };
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
        CRASH_REPORTING = const CRASH_REPORTING as u32,
        plat_crash_console_init = sym PlatformImpl::crash_console_init,
        plat_crash_console_putc = sym PlatformImpl::crash_console_putc,
        plat_crash_console_flush = sym PlatformImpl::crash_console_flush,
    );
}
