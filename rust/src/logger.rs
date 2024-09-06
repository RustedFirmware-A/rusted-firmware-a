// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::fmt::Write;
#[cfg(not(test))]
use core::panic::PanicInfo;
use log::{LevelFilter, Log, Metadata, Record, SetLoggerError};
use pl011_uart::Uart;
use spin::{mutex::SpinMutex, Once};

static LOGGER: Once<Logger> = Once::new();

struct Logger {
    uart: SpinMutex<Uart>,
}

impl Log for Logger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(self.uart.lock(), "{}: {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}

/// Initialises UART logger.
pub fn init(uart: Uart, max_level: LevelFilter) -> Result<(), SetLoggerError> {
    let logger = LOGGER.call_once(|| Logger {
        uart: SpinMutex::new(uart),
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
        let _ = writeln!(logger.uart.lock(), "{}", info);
    }
    loop {}
}
