// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Implementation of a subset of the Arm semihosting calls.
//! See <https://github.com/ARM-software/abi-aa/blob/main/semihosting/semihosting.rst>

#[cfg(not(any(test, feature = "fakes")))]
use core::arch::asm;

/// `SYS_*` operation codes from the semihosting spec.
#[allow(unused)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
enum Operation {
    Open = 0x01,
    Close = 0x02,
    Write0 = 0x04,
    Writec = 0x03,
    Write = 0x05,
    Read = 0x06,
    Readc = 0x07,
    Seek = 0x0A,
    Flen = 0x0C,
    Remove = 0x0E,
    Clock = 0x10,
    Time = 0x11,
    System = 0x12,
    Errno = 0x13,
    Exit = 0x18,
    ExitExtended = 0x20,
    Elapsed = 0x30,
    Tickfreq = 0x31,
}

/// Makes a semihosting call with the given operation code and parameters.
///
/// # Safety
///
/// `system_block_address` must be a valid pointer to an argument block of the appropriate length
/// for the operation being called.
#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
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
    /// Hardware event: branch through zero
    BranchThroughZero = 0x20000,
    /// Hardware event: undefined instruction
    UndefinedInstr = 0x20001,
    /// Hardware event: software interrupt
    SoftwareInterrupt = 0x20002,
    /// Hardware event: prefetch abort
    PrefetchAbort = 0x20003,
    /// Hardware event: data abort
    DataAbort = 0x20004,
    /// Hardware event: address exception
    AddressException = 0x20005,
    /// Hardware event: IRQ
    Irq = 0x20006,
    /// Hardware event: FIQ
    Fiq = 0x20007,
    /// Software event: breakpoint
    BreakPoint = 0x20020,
    /// Software event: watchpoint
    WatchPoint = 0x20021,
    /// Software event: step complete
    StepComplete = 0x20022,
    /// Software event: unknown runtime error
    RunTimeErrorUnknown = 0x20023,
    /// Software event: internal error
    InternalError = 0x20024,
    /// Software event: user interruption
    UserInterruption = 0x20025,
    /// Software event: application exit
    ApplicationExit = 0x20026,
    /// Software event: stack overflow
    StackOverflow = 0x20027,
    /// Software event: division by zero
    DivisionByZero = 0x20028,
    /// Software event: operating system specific
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
#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
pub fn semihosting_exit(reason: AdpStopped, subcode: u64) {
    let parameters: [u64; 2] = [reason.into(), subcode];
    // SAFETY: The `parameters` pointer is valid, and contains two parameters as expected by
    // `SYS_EXIT`.
    unsafe {
        semihosting_call(Operation::Exit, parameters.as_ptr());
    }
}
