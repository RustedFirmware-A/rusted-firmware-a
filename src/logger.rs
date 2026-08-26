// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Traits and implementations for loggers.

pub mod inmemory;

use core::{
    fmt::{Arguments, Write},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use log::{Log, Metadata, Record, SetLoggerError};
use spin::{Once, mutex::SpinMutex};

/// Wrapper around `Logger` to be stored in a static variable.
#[derive(Default)]
pub struct OnceLogger<LogSinkImpl> {
    logger: Once<Logger<LogSinkImpl>>,
}

impl<LogSinkImpl: LogSink> OnceLogger<LogSinkImpl> {
    /// Constructs a new uninitialised `OnceLogger`.
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
    pub const fn new(primary: P, secondary: S) -> Self {
        Self {
            primary,
            secondary,
            secondary_enabled: AtomicBool::new(true),
        }
    }

    /// Enables or disables writing logs to the secondary logger.
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

/// A [`LogSink`] decorator that prepends timestamps to log messages.
///
/// If the timestamp function returns `Some(duration)`, the duration is formatted
/// as seconds and microseconds: `[{:>4}.{:06}] <message>`. If `None` is returned,
/// the message is passed through without modification.
///
/// # Example
///
/// ```
/// use core::fmt::Write;
/// use core::time::Duration;
/// use rf_a_bl31::logger::{LogSink, LockedWriter, TimestampedLogger};
///
/// struct FakeUart;
/// impl Write for FakeUart {
///     fn write_str(&mut self, _s: &str) -> core::fmt::Result {
///         Ok(())
///     }
/// }
///
/// let sink = LockedWriter::new(FakeUart);
/// let logger = TimestampedLogger::new(sink, || Some(Duration::from_micros(1_234_567)));
///
/// // Prepends timestamp: "[   1.234567] System initialized\n"
/// logger.write_fmt(format_args!("System initialized\n"));
/// ```
pub struct TimestampedLogger<S: LogSink, F: Fn() -> Option<Duration>> {
    sink: S,
    get_timestamp: F,
}

impl<S: LogSink, F: Fn() -> Option<Duration>> TimestampedLogger<S, F> {
    /// Creates a new `TimestampedLogger` wrapping the given [`LogSink`].
    pub const fn new(sink: S, get_timestamp: F) -> Self {
        Self {
            sink,
            get_timestamp,
        }
    }

    /// Gets a reference to the inner log sink.
    pub fn inner(&self) -> &S {
        &self.sink
    }
}

impl<S: LogSink, F: Fn() -> Option<Duration> + Send + Sync> LogSink for TimestampedLogger<S, F> {
    fn write_fmt(&self, args: Arguments) {
        if let Some(timestamp) = (self.get_timestamp)() {
            let seconds = timestamp.as_secs();
            let microseconds = timestamp.subsec_micros();
            self.sink.write_fmt(format_args!(
                "[{:>4}.{:06}] {}",
                seconds, microseconds, args
            ));
        } else {
            self.sink.write_fmt(args);
        }
    }

    fn flush(&self) {
        self.sink.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;
    use spin::Mutex;

    struct BufferSink(Mutex<String>);

    impl LogSink for BufferSink {
        fn write_fmt(&self, args: Arguments) {
            self.0.lock().write_fmt(args).unwrap();
        }

        fn flush(&self) {}
    }

    #[test]
    fn test_timestamped_logger_with_timestamp() {
        let sink = BufferSink(Mutex::new(String::new()));
        let logger = TimestampedLogger::new(sink, || Some(Duration::from_micros(1_234_567)));

        logger.write_fmt(format_args!("Test message\n"));

        assert_eq!(
            logger.inner().0.lock().as_str(),
            "[   1.234567] Test message\n"
        );
    }

    #[test]
    fn test_timestamped_logger_without_timestamp() {
        let sink = BufferSink(Mutex::new(String::new()));
        let logger = TimestampedLogger::new(sink, || None);

        logger.write_fmt(format_args!("Test message\n"));

        assert_eq!(logger.inner().0.lock().as_str(), "Test message\n");
    }
}
