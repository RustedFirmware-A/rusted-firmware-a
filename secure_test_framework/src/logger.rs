// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::fmt::Write;
use log::{Log, Metadata, Record, SetLoggerError};
use percore::{ExceptionLock, exception_free};
use spin::{Once, mutex::SpinMutex};

static LOGGER: Once<Logger> = Once::new();

struct Logger {
    console: ExceptionLock<SpinMutex<&'static mut (dyn Write + Send)>>,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        exception_free(|token| {
            writeln!(
                self.console.borrow(token).lock(),
                "Test {}: {}",
                record.level(),
                record.args()
            )
            .unwrap();
        });
    }

    fn flush(&self) {}
}

/// Initialises UART logger.
pub fn init(console: &'static mut (dyn Write + Send)) -> Result<(), SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger {
        console: ExceptionLock::new(SpinMutex::new(console)),
    });
    log::set_logger(logger)?;
    // Init the maximum log level to the statically configured maximum level controlled by the
    // `max_log_<level>` Cargo feature flag.
    log::set_max_level(log::STATIC_MAX_LEVEL);
    Ok(())
}
