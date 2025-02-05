// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Implementation of a subset of the Arm semihosting calls.
//! See https://github.com/ARM-software/abi-aa/blob/main/semihosting/semihosting.rst.

#[cfg(not(test))]
use core::arch::asm;

/// `SYS_*` operation codes from the semihosting spec.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
enum Operation {
    #[allow(unused)]
    Open = 0x01,
    #[allow(unused)]
    Close = 0x02,
    #[allow(unused)]
    Write0 = 0x04,
    #[allow(unused)]
    Writec = 0x03,
    #[allow(unused)]
    Write = 0x05,
    #[allow(unused)]
    Read = 0x06,
    #[allow(unused)]
    Readc = 0x07,
    #[allow(unused)]
    Seek = 0x0A,
    #[allow(unused)]
    Flen = 0x0C,
    #[allow(unused)]
    Remove = 0x0E,
    #[allow(unused)]
    Clock = 0x10,
    #[allow(unused)]
    Time = 0x11,
    #[allow(unused)]
    System = 0x12,
    #[allow(unused)]
    Errno = 0x13,
    Exit = 0x18,
    #[allow(unused)]
    ExitExtended = 0x20,
    #[allow(unused)]
    Elapsed = 0x30,
    #[allow(unused)]
    Tickfreq = 0x31,
}

/// Makes a semihosting call with the given operation code and parameters.
///
/// # Safety
///
/// `system_block_address` must be a valid pointer to an argument block of the appropriate length
/// for the operation being called.
#[cfg(target_arch = "aarch64")]
unsafe fn semihosting_call(operation: Operation, system_block_address: *const u64) -> u64 {
    let result;
    // SAFETY: The caller guarantees that `system_block_address` is valid and points to enough
    // memory for `operation`.
    unsafe {
        asm!(
            "hlt #0xf000",
            in("w0") operation as u32,
            inout("x1") system_block_address => _,
            lateout("x0") result,
            options(nostack)
        );
    }
    result
}

/// Reason codes for a `SYS_EXIT` call.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AdpStopped {
    BranchThroughZero = 0x20000,
    UndefinedInstr = 0x20001,
    SoftwareInterrupt = 0x20002,
    PrefetchAbort = 0x20003,
    DataAbort = 0x20004,
    AddressException = 0x20005,
    Irq = 0x20006,
    Fiq = 0x20007,
    BreakPoint = 0x20020,
    WatchPoint = 0x20021,
    StepComplete = 0x20022,
    RunTimeErrorUnknown = 0x20023,
    InternalError = 0x20024,
    UserInterruption = 0x20025,
    ApplicationExit = 0x20026,
    StackOverflow = 0x20027,
    DivisionByZero = 0x20028,
    OSSpecific = 0x20029,
}

impl From<AdpStopped> for u32 {
    fn from(value: AdpStopped) -> Self {
        value as u32
    }
}

impl From<AdpStopped> for u64 {
    fn from(value: AdpStopped) -> Self {
        (value as u32).into()
    }
}

/// Reports an exception to the debugger.
///
/// The most common use is to report that execution has completed, with reason
/// `AdpStopped::ApplicationExit`.
#[cfg(target_arch = "aarch64")]
pub fn semihosting_exit(reason: AdpStopped, subcode: u64) {
    let parameters: [u64; 2] = [reason.into(), subcode];
    // SAFETY: The `parameters` pointer is valid, and contains two parameters as expected by
    // `SYS_EXIT`.
    unsafe {
        semihosting_call(Operation::Exit, parameters.as_ptr());
    }
}
