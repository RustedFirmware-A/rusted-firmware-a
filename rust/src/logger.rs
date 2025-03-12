// Copyright (c) 2023, Google LLC. All rights reserved.
// Copyright (c) 2025, Arm Ltd. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::platform::LoggerWriter;
use core::fmt::Write;
#[cfg(not(test))]
use core::panic::PanicInfo;
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
pub fn init(writer: LoggerWriter, max_level: LevelFilter) -> Result<(), SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger {
        writer: SpinMutex::new(writer),
    });
    log::set_logger(logger)?;
    log::set_max_level(max_level);
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
