// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod inmemory;

use crate::{debug::DEBUG, platform::LogSinkImpl};
use core::{
    fmt::{Arguments, Write},
    sync::atomic::{AtomicBool, Ordering},
};
#[cfg(not(test))]
use core::{option_env, panic::PanicInfo};
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::{Once, mutex::SpinMutex};

static LOGGER: Once<Logger> = Once::new();

struct Logger {
    sink: LogSinkImpl,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(self.sink, "{}: {}", record.level(), record.args());
    }

    fn flush(&self) {}
}

/// Initialises logger.
pub fn init(sink: LogSinkImpl) -> Result<(), SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger { sink });
    log::set_logger(logger)?;
    log::set_max_level(build_time_log_level());
    Ok(())
}

/// Gets a reference to the log sink, if it has been set.
#[allow(unused)]
pub fn get_log_sink() -> Option<&'static LogSinkImpl> {
    LOGGER.get().map(|logger| &logger.sink)
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(sink) = get_log_sink() {
        writeln!(sink, "{}", info);
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
                LevelFilter::Debug
            } else {
                LevelFilter::Info
            }
        }
    }
}

/// Something to which logs can be sent.
///
/// Note that unlike `core::fmt::Write`, the `write_fmt` method on this trait takes `&self` rather
/// than `&mut self`. This means that the implementation is responsible for handling locking if
/// necessary, or can be made lock-free.
pub trait LogSink {
    /// Writes the given format arguments to the log sink.
    fn write_fmt(&self, args: Arguments);
}

/// An implementation of `LogSink` that wraps around any implementation of `core::fmt::Write`.
///
/// This wraps the given writer in a spin mutex, to allow a single instance it to be used safely
/// from multiple cores. This also ensures that a complete log line is written at once, rather than
/// being interleaved with characters from another core.
pub struct LockedWriter<W: Write> {
    writer: SpinMutex<W>,
}

impl<W: Write> LockedWriter<W> {
    /// Creates a new `LockedWriter` wrapping the given [`Write`] implementation.
    #[allow(unused)]
    pub const fn new(writer: W) -> Self {
        Self {
            writer: SpinMutex::new(writer),
        }
    }
}

impl<W: Write> LogSink for LockedWriter<W> {
    fn write_fmt(&self, args: Arguments) {
        // Ignore errors.
        let _ = self.writer.lock().write_fmt(args);
    }
}

/// A logger which will always log to a primary sink, and optionally also to a secondary sink.
///
/// For example, the primary sink could be a per-core memory buffer, and the secondary sink a UART.
/// Writing to the UART requires taking a mutex, but writing to the per-core memory buffer does not.
/// This means that when the UART is disabled, logging is lock-free and should never block.
pub struct HybridLogger<P: LogSink, S: LogSink> {
    primary: P,
    secondary: S,
    secondary_enabled: AtomicBool,
}

impl<P: LogSink, S: LogSink> HybridLogger<P, S> {
    /// Creates a new logger with the given primary and secondary log sinks.
    ///
    /// Logging to the secondary sink will initially be enabled.
    #[allow(unused)]
    pub const fn new(primary: P, secondary: S) -> Self {
        Self {
            primary,
            secondary,
            secondary_enabled: AtomicBool::new(true),
        }
    }

    /// Enables or disables writing logs to the secondary logger.
    #[allow(unused)]
    pub fn enable_secondary(&self, enable: bool) {
        self.secondary_enabled.store(enable, Ordering::Release);
    }
}

impl<P: LogSink, S: LogSink> LogSink for HybridLogger<P, S> {
    fn write_fmt(&self, args: Arguments) {
        self.primary.write_fmt(args);
        if self.secondary_enabled.load(Ordering::Acquire) {
            self.secondary.write_fmt(args);
        }
    }
}
