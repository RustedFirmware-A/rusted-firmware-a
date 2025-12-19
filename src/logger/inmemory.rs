// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::PerCoreState,
    logger::LogSink,
    pagetable::flush_dcache,
    platform::{Platform, PlatformImpl, exception_free},
};
use core::{
    cell::RefCell,
    cmp::min,
    fmt::{self, Arguments, Write},
};
use percore::{ExceptionLock, PerCore};
use zerocopy::{FromBytes, Immutable, KnownLayout};

/// An in-memory logger with a circular buffer.
#[derive(FromBytes, KnownLayout, Immutable)]
#[repr(C)]
pub struct MemoryLogger<const BUFFER_SIZE: usize> {
    /// The position in `buffer` at which to write the next byte.
    next_offset: usize,
    /// The total number of bytes logged since the logger was created or reset. Note that this may
    /// be greater than `BUFFER_SIZE`, so not all the bytes logged may still be available.
    logged_bytes_count: usize,
    buffer: [u8; BUFFER_SIZE],
}

impl<const BUFFER_SIZE: usize> MemoryLogger<BUFFER_SIZE> {
    /// Creates a new in-memory logger with a zeroed-out circular buffer.
    #[allow(unused)]
    pub const fn new() -> Self {
        Self {
            next_offset: 0,
            logged_bytes_count: 0,
            buffer: [0; BUFFER_SIZE],
        }
    }

    /// Resets the logger to an empty state.
    #[allow(unused)]
    pub fn reset(&mut self) {
        self.next_offset = 0;
        self.logged_bytes_count = 0;
    }

    /// Adds the given bytes to the circular buffer.
    ///
    /// If more bytes are passed than can fit in the buffer at once, then the initial bytes are ignored.
    fn add_bytes(&mut self, mut bytes: &[u8]) {
        self.logged_bytes_count += bytes.len();
        // If we are given more bytes than we can fit, keep the end.
        if bytes.len() > BUFFER_SIZE {
            bytes = &bytes[bytes.len() - BUFFER_SIZE..];
        }

        let buffer_end_len = min(bytes.len(), BUFFER_SIZE - self.next_offset);
        self.buffer[self.next_offset..self.next_offset + buffer_end_len]
            .copy_from_slice(&bytes[0..buffer_end_len]);
        self.buffer[0..bytes.len() - buffer_end_len].copy_from_slice(&bytes[buffer_end_len..]);
        self.next_offset = (self.next_offset + bytes.len()) % BUFFER_SIZE;
    }

    /// Flushes the contents of the logger from the data cache.
    pub fn flush(&self) {
        flush_dcache(self);
    }

    /// Shifts the wrapped contents of the ring buffer so that the oldest byte
    /// is at index 0.
    fn shift_to_start(&mut self) {
        if self.logged_bytes_count < BUFFER_SIZE {
            return;
        }

        // self.next_offset points to the oldest byte.
        self.buffer.rotate_left(self.next_offset);

        // The oldest byte is now at index 0.
        self.next_offset = 0;
    }

    /// Finds a valid UTF-8 suffix and returns a reference to it.
    #[allow(unused)]
    pub fn as_str(&mut self) -> &str {
        self.shift_to_start();

        let mut current = &self.buffer[..min(BUFFER_SIZE, self.logged_bytes_count)];

        // After `shift_to_start` the buffer may start with a partially overwritten UTF-8 character
        // (at most 3 bytes are corrupt).
        // A single call to `str::from_utf8` may identify only a portion of the corrupt sequence, so
        // to find the valid suffix we need to loop a few times.
        loop {
            match str::from_utf8(current) {
                Ok(valid_str) => return valid_str,
                Err(e) => {
                    // The error is at index 0.
                    match e.error_len() {
                        Some(len) => {
                            // This byte sequence is corrupt. Skip it and retry.
                            current = &current[len..];
                        }
                        None => {
                            // Only an incomplete character remains at the end.
                            return "";
                        }
                    }
                }
            }
        }
    }
}

impl<const BUFFER_SIZE: usize> Default for MemoryLogger<BUFFER_SIZE> {
    fn default() -> Self {
        Self {
            next_offset: 0,
            logged_bytes_count: 0,
            buffer: [0; BUFFER_SIZE],
        }
    }
}

impl<const BUFFER_SIZE: usize> Write for MemoryLogger<BUFFER_SIZE> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.add_bytes(s.as_bytes());
        Ok(())
    }
}

/// A per-core in-memory logger.
pub struct PerCoreMemoryLogger<'a, const BUFFER_SIZE: usize> {
    logs: PerCoreState<&'a mut MemoryLogger<BUFFER_SIZE>>,
}

impl<'a, const BUFFER_SIZE: usize> PerCoreMemoryLogger<'a, BUFFER_SIZE> {
    #[allow(unused)]
    pub fn new(loggers: [&'a mut MemoryLogger<BUFFER_SIZE>; PlatformImpl::CORE_COUNT]) -> Self {
        Self {
            logs: PerCore::new(loggers.map(|logger| ExceptionLock::new(RefCell::new(logger)))),
        }
    }
}

impl<const BUFFER_SIZE: usize> LogSink for PerCoreMemoryLogger<'_, BUFFER_SIZE> {
    fn write_fmt(&self, args: Arguments) {
        // The `MemoryLogger` should never return an error.
        let _ = exception_free(|token| self.logs.get().borrow_mut(token).write_fmt(args));
    }

    fn flush(&self) {
        exception_free(|token| self.logs.get().borrow_mut(token).flush());
    }
}

impl<const BUFFER_SIZE: usize> LogSink for &PerCoreMemoryLogger<'_, BUFFER_SIZE> {
    fn write_fmt(&self, args: Arguments) {
        (*self).write_fmt(args)
    }

    fn flush(&self) {
        (*self).flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_logger_no_wrap() {
        let mut logger = MemoryLogger::<5>::new();

        logger.add_bytes(&[1]);
        assert_eq!(logger.next_offset, 1);
        assert_eq!(logger.buffer, [1, 0, 0, 0, 0]);

        logger.add_bytes(&[2, 3]);
        assert_eq!(logger.next_offset, 3);
        assert_eq!(logger.buffer, [1, 2, 3, 0, 0]);
    }

    #[test]
    fn memory_logger_too_long() {
        let mut logger = MemoryLogger::<5>::new();

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
        let mut logger = MemoryLogger::<5>::new();

        logger.add_bytes(&[1, 2, 3]);
        assert_eq!(logger.next_offset, 3);
        assert_eq!(logger.buffer, [1, 2, 3, 0, 0]);

        logger.add_bytes(&[4, 5, 6]);
        assert_eq!(logger.next_offset, 1);
        assert_eq!(logger.buffer, [6, 2, 3, 4, 5]);
    }

    #[test]
    fn memory_logger_boundary() {
        let mut logger = MemoryLogger::<5>::new();

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

    #[test]
    fn memory_logger_shift_to_start() {
        let mut logger = MemoryLogger::<5>::new();

        logger.shift_to_start();
        assert_eq!(logger.buffer, [0, 0, 0, 0, 0]);

        logger.add_bytes(&[0, 1, 2, 3]);
        logger.shift_to_start();
        assert_eq!(logger.buffer, [0, 1, 2, 3, 0]);

        logger.add_bytes(&[4, 5, 6]);
        assert_eq!(logger.buffer, [5, 6, 2, 3, 4]);

        logger.shift_to_start();
        assert_eq!(logger.buffer, [2, 3, 4, 5, 6]);
    }

    #[test]
    fn memory_logger_to_str() {
        let mut logger = MemoryLogger::<16>::new();

        logger.add_bytes("abcdefgh".as_bytes());
        assert_eq!(logger.as_str(), "abcdefgh");

        logger.add_bytes("334455667788".as_bytes());
        assert_eq!(logger.as_str(), "efgh334455667788");

        logger.add_bytes("abcdefgh".as_bytes());
        logger.add_bytes("abcdefgh".as_bytes());
        logger.add_bytes("abcdefgh".as_bytes());
        logger.add_bytes("abcdefgh".as_bytes());
        assert_eq!(logger.as_str(), "abcdefghabcdefgh");
    }

    #[test]
    fn memory_logger_corrupt_utf8() {
        let mut logger = MemoryLogger::<16>::new();

        // Crab emoji is 4 bytes long in UTF-8.

        logger.add_bytes("🦀🦀🦀🦀".as_bytes());
        assert_eq!(logger.as_str(), "🦀🦀🦀🦀");

        logger.add_bytes("a".as_bytes());
        assert_eq!(logger.as_str(), "🦀🦀🦀a");

        logger.add_bytes("a".as_bytes());
        assert_eq!(logger.as_str(), "🦀🦀🦀aa");

        logger.add_bytes("a".as_bytes());
        assert_eq!(logger.as_str(), "🦀🦀🦀aaa");

        logger.add_bytes("a".as_bytes());
        assert_eq!(logger.as_str(), "🦀🦀🦀aaaa");
    }
}
