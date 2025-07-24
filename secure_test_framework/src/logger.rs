// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::fmt::Write;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::mutex::SpinMutex;

static LOGGER: Logger = Logger {
    console: SpinMutex::new(None),
};

struct Logger {
    console: SpinMutex<Option<&'static mut (dyn Write + Send)>>,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(
            self.console.lock().as_mut().unwrap(),
            "Test {}: {}",
            record.level(),
            record.args()
        )
        .unwrap();
    }

    fn flush(&self) {}
}

/// Initialises UART logger.
pub fn init(console: &'static mut (dyn Write + Send)) -> Result<(), SetLoggerError> {
    LOGGER.console.lock().replace(console);
    log::set_logger(&LOGGER)?;
    log::set_max_level(build_time_log_level());
    Ok(())
}

/// Returns the logging [`LevelFilter`] set by the build-time environment variable `STF_LOG_LEVEL`.
/// `STF_LOG_LEVEL` can have the lower-case string values "off", "error", "warn", "info", "debug", or
/// "trace", corresponding to the named values of [`LevelFilter`]. If `STF_LOG_LEVEL` is absent or has
/// some other value, this function returns `LevelFilter::Debug`.
pub const fn build_time_log_level() -> LevelFilter {
    let level = match option_env!("STF_LOG_LEVEL") {
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
        _ => LevelFilter::Debug,
    }
}
