// Copyright (c) 2023, Google LLC. All rights reserved.
// Copyright (c) 2025, Arm Ltd. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{debug::DEBUG, platform::LoggerWriter};
use core::fmt::Write;
#[cfg(not(test))]
use core::{option_env, panic::PanicInfo};
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::{mutex::SpinMutex, Once};

static LOGGER: Once<Logger> = Once::new();

struct Logger {
    writer: SpinMutex<LoggerWriter>,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(self.writer.lock(), "{}: {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}

/// Initialises logger.
pub fn init(writer: LoggerWriter) -> Result<(), SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger {
        writer: SpinMutex::new(writer),
    });
    log::set_logger(logger)?;
    log::set_max_level(build_time_log_level());
    Ok(())
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(logger) = LOGGER.get() {
        // Ignore any errors writing to the UART, to avoid panicking recursively.
        let _ = writeln!(logger.writer.lock(), "{}", info);
    }
    loop {}
}

/// Returns the logging [`LevelFilter`] set by the build-time environment variable `LOG_LEVEL`.
/// `LOG_LEVEL` can have the lower-case string values "off", "error", "warn", "info", "debug", or
/// "trace", corresponding to the named values of [`LevelFilter`]. If `LOG_LEVEL` is absent or has
/// some other value, this function returns `LevelFilter::Trace` if [`DEBUG`] is true, otherwise
/// `LevelFilter::Info`.
pub const fn build_time_log_level() -> LevelFilter {
    let level = match option_env!("LOG_LEVEL") {
        Some(level) => level,
        None => "",
    };
    match level.as_bytes() {
        b"off" => LevelFilter::Off,
        b"error" => LevelFilter::Error,
        b"warn" => LevelFilter::Warn,
        b"info" => LevelFilter::Info,
        b"debug" => LevelFilter::Debug,
        b"trace" => LevelFilter::Trace,
        _ => {
            if DEBUG {
                LevelFilter::Trace
            } else {
                LevelFilter::Info
            }
        }
    }
}
