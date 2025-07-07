// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::PerCoreState,
    logger::LogSink,
    platform::{Platform, PlatformImpl, exception_free},
};
use core::{
    cell::RefCell,
    cmp::min,
    fmt::{self, Arguments, Write},
};
use percore::{ExceptionLock, PerCore};

/// An in-memory logger with a circular buffer.
pub struct MemoryLogger<'a> {
    buffer: &'a mut [u8],
    next_offset: usize,
}

impl<'a> MemoryLogger<'a> {
    /// Creates a new in-memory logger with a zeroed-out circular buffer.
    pub const fn new(buffer: &'a mut [u8]) -> Self {
        Self {
            buffer,
            next_offset: 0,
        }
    }

    /// Adds the given bytes to the circular buffer.
    ///
    /// If more bytes are passed than can fit in the buffer at once, then the initial bytes are ignored.
    fn add_bytes(&mut self, mut bytes: &[u8]) {
        // If we are given more bytes than we can fit, keep the end.
        if bytes.len() > self.buffer.len() {
            bytes = &bytes[bytes.len() - self.buffer.len()..];
        }

        let buffer_end_len = min(bytes.len(), self.buffer.len() - self.next_offset);
        self.buffer[self.next_offset..self.next_offset + buffer_end_len]
            .copy_from_slice(&bytes[0..buffer_end_len]);
        self.buffer[0..bytes.len() - buffer_end_len].copy_from_slice(&bytes[buffer_end_len..]);
        self.next_offset = (self.next_offset + bytes.len()) % self.buffer.len();
    }
}

impl Write for MemoryLogger<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.add_bytes(s.as_bytes());
        Ok(())
    }
}

/// A per-core in-memory logger.
pub struct PerCoreMemoryLogger<'a> {
    logs: PerCoreState<MemoryLogger<'a>>,
}

impl<'a> PerCoreMemoryLogger<'a> {
    #[allow(unused)]
    pub fn new(buffers: [&'a mut [u8]; PlatformImpl::CORE_COUNT]) -> Self {
        Self {
            logs: PerCore::new(
                buffers.map(|buffer| ExceptionLock::new(RefCell::new(MemoryLogger::new(buffer)))),
            ),
        }
    }
}

impl LogSink for PerCoreMemoryLogger<'_> {
    fn write_fmt(&self, args: Arguments) {
        // The `MemoryLogger` should never return an error.
        let _ = exception_free(|token| self.logs.get().borrow_mut(token).write_fmt(args));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_logger_no_wrap() {
        let mut buffer = [0; 5];
        let mut logger = MemoryLogger::new(&mut buffer);

        logger.add_bytes(&[1]);
        assert_eq!(logger.next_offset, 1);
        assert_eq!(logger.buffer, [1, 0, 0, 0, 0]);

        logger.add_bytes(&[2, 3]);
        assert_eq!(logger.next_offset, 3);
        assert_eq!(logger.buffer, [1, 2, 3, 0, 0]);
    }

    #[test]
    fn memory_logger_too_long() {
        let mut buffer = [0; 5];
        let mut logger = MemoryLogger::new(&mut buffer);

        logger.add_bytes(&[1, 2, 3, 4, 5, 6]);
        assert_eq!(logger.next_offset, 0);
        assert_eq!(logger.buffer, [2, 3, 4, 5, 6]);

        logger.add_bytes(&[7, 8]);
        assert_eq!(logger.next_offset, 2);
        assert_eq!(logger.buffer, [7, 8, 4, 5, 6]);

        logger.add_bytes(&[9, 10, 11, 12, 13]);
        assert_eq!(logger.next_offset, 2);
        assert_eq!(logger.buffer, [12, 13, 9, 10, 11]);
    }

    #[test]
    fn memory_logger_wrap() {
        let mut buffer = [0; 5];
        let mut logger = MemoryLogger::new(&mut buffer);

        logger.add_bytes(&[1, 2, 3]);
        assert_eq!(logger.next_offset, 3);
        assert_eq!(logger.buffer, [1, 2, 3, 0, 0]);

        logger.add_bytes(&[4, 5, 6]);
        assert_eq!(logger.next_offset, 1);
        assert_eq!(logger.buffer, [6, 2, 3, 4, 5]);
    }

    #[test]
    fn memory_logger_boundary() {
        let mut buffer = [0; 5];
        let mut logger = MemoryLogger::new(&mut buffer);

        logger.add_bytes(&[1, 2, 3]);
        assert_eq!(logger.next_offset, 3);
        assert_eq!(logger.buffer, [1, 2, 3, 0, 0]);

        logger.add_bytes(&[4, 5]);
        assert_eq!(logger.next_offset, 0);
        assert_eq!(logger.buffer, [1, 2, 3, 4, 5]);

        logger.add_bytes(&[6, 7, 8, 9, 10]);
        assert_eq!(logger.next_offset, 0);
        assert_eq!(logger.buffer, [6, 7, 8, 9, 10]);
    }
}
