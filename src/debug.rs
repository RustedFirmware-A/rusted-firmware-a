// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Debug output.

#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
use include_first::include_first;

/// True if the build is configured with debug assertions on.
#[cfg(not(any(test, feature = "fakes")))]
pub const DEBUG: bool = cfg!(debug_assertions);

/// Whether to enable assertions in assembly code.
#[cfg(not(any(test, feature = "fakes")))]
pub const ENABLE_ASSERTIONS: bool = DEBUG;

/// Whether to enable crash reporting in assembly code.
// TODO: Should this be configurable separately from `DEBUG`?
#[cfg(not(any(test, feature = "fakes")))]
pub const CRASH_REPORTING: bool = DEBUG;

/// The number of registers which can be saved in the crash buffer.
const CRASH_BUFFER_REGISTER_COUNT: usize = 8;

/// A buffer used by the assembly crash dumping code to store registers to dump.
#[derive(Clone, Debug)]
#[repr(C, align(64))]
pub struct CrashBuffer([u64; CRASH_BUFFER_REGISTER_COUNT]);

impl CrashBuffer {
    /// An empty instance of the crash buffer, for initialising statics.
    pub const EMPTY: Self = Self([0; CRASH_BUFFER_REGISTER_COUNT]);
}

/// Generates a `global_asm!` block for debug-related assembly code.
#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
#[macro_export]
#[include_first]
macro_rules! debug_asm {
    ($platform:ty) => {
        type PlatformImplDebug_ = $platform;

        mod debug_asm {
            use super::PlatformImplDebug_ as PlatformImpl;
            use $crate::platform::Platform;

            core::arch::global_asm!(
                include_str!("asm_macros_common.S"),
                include_str!("crash_reporting.S"),
                include_str!("debug.S"),
                include_str!("asm_macros_common_purge.S"),
                CRASH_REPORTING = const $crate::debug::CRASH_REPORTING as u32,
                DEBUG = const $crate::debug::DEBUG as u32,
                LOG_LEVEL_DEBUG = const $crate::reexports::log::LevelFilter::Debug as u32,
                LOG_LEVEL = const $crate::reexports::log::STATIC_MAX_LEVEL as u32,
                ENABLE_ASSERTIONS = const $crate::debug::ENABLE_ASSERTIONS as u32,
                ENABLE_PAUTH = const cfg!(feature = "pauth") as u32,
                MODE_SP_ELX = const 1,
                CPU_DATA_CRASH_BUFFER_OFFSET = const core::mem::offset_of!($crate::context::CpuData, crash_buffer),
                CRASH_BUFFER_SIZE = const size_of::<$crate::debug::CrashBuffer>(),
                REGSZ = const size_of::<u64>(),
                plat_crash_console_init = sym PlatformImpl::crash_console_init,
                plat_crash_console_putc = sym PlatformImpl::crash_console_putc,
                plat_crash_console_flush = sym PlatformImpl::crash_console_flush,
                plat_crash_print_regs = sym PlatformImpl::dump_registers,
                plat_panic_handler = sym PlatformImpl::panic_handler,
                cpu_dump_registers = sym $crate::cpu::cpu_dump_registers::<PlatformImpl>,
            );
        }
    };
}
#[allow(clippy::single_component_path_imports)]
#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
pub use debug_asm;
