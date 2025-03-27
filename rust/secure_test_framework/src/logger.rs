// Copyright (c) 2025, Google LLC. All rights reserved.
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
pub fn init(
    console: &'static mut (dyn Write + Send),
    max_level: LevelFilter,
) -> Result<(), SetLoggerError> {
    LOGGER.console.lock().replace(console);
    log::set_logger(&LOGGER)?;
    log::set_max_level(max_level);
    Ok(())
}
