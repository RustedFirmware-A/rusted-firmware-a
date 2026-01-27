// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod inmemory;

use crate::platform::LogSinkImpl;
#[cfg(not(test))]
use core::panic::PanicInfo;
use core::{
    fmt::{Arguments, Write},
    sync::atomic::{AtomicBool, Ordering},
};
use log::{Log, Metadata, Record, SetLoggerError};
use spin::{Once, mutex::SpinMutex};

pub static LOGGER: OnceLogger<LogSinkImpl> = OnceLogger::new();

pub struct OnceLogger<LogSinkImpl> {
    logger: Once<Logger<LogSinkImpl>>,
}

impl<LogSinkImpl: LogSink> OnceLogger<LogSinkImpl> {
    pub const fn new() -> Self {
        Self {
            logger: Once::new(),
        }
    }

    /// Initialises logger.
    pub fn init(&'static self, sink: LogSinkImpl) -> Result<(), SetLoggerError> {
        let logger = self.logger.call_once(|| Logger { sink });
        log::set_logger(logger)?;
        // Init the maximum log level to the statically configured maximum level controlled by the
        // `max_log_<level>` Cargo feature flag.
        log::set_max_level(log::STATIC_MAX_LEVEL);
        Ok(())
    }

    /// Gets a reference to the log sink, if it has been set.
    #[allow(unused)]
    pub fn log_sink(&self) -> Option<&LogSinkImpl> {
        self.logger.get().map(|logger| &logger.sink)
    }
}

struct Logger<LogSinkImpl> {
    sink: LogSinkImpl,
}

impl<LogSinkImpl: LogSink> Log for Logger<LogSinkImpl> {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(self.sink, "{}: {}", record.level(), record.args());
    }

    fn flush(&self) {
        self.sink.flush();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    if let Some(sink) = LOGGER.log_sink() {
        writeln!(sink, "{info}");
    }
    loop {}
}

/// Something to which logs can be sent.
///
/// Note that unlike `core::fmt::Write`, the `write_fmt` method on this trait takes `&self` rather
/// than `&mut self`. This means that the implementation is responsible for handling locking if
/// necessary, or can be made lock-free.
pub trait LogSink: Send + Sync {
    /// Writes the given format arguments to the log sink.
    fn write_fmt(&self, args: Arguments);

    /// Flushes any in-progress logs.
    fn flush(&self);
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

impl<W: Send + Sync + Write> LogSink for LockedWriter<W> {
    fn write_fmt(&self, args: Arguments) {
        // Ignore errors.
        let _ = self.writer.lock().write_fmt(args);
    }

    fn flush(&self) {}
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

    fn flush(&self) {
        self.primary.flush();
        self.secondary.flush();
    }
}
