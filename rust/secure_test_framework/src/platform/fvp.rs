// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use core::{fmt::Write, ptr::NonNull};
use spin::{
    Once,
    mutex::{SpinMutex, SpinMutexGuard},
};

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: NonNull<PL011Registers> = NonNull::new(0x1C09_0000 as _).unwrap();

static UART: Once<SpinMutex<Uart>> = Once::new();

pub struct Fvp;

impl Platform for Fvp {
    fn make_log_sink() -> &'static mut (dyn Write + Send) {
        let uart = UART.call_once(|| {
            // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
            // accesses that address range.
            SpinMutex::new(Uart::new(unsafe {
                UniqueMmioPointer::new(PL011_BASE_ADDRESS)
            }))
        });
        let uart: &'static mut Uart = SpinMutexGuard::leak(uart.lock());
        uart
    }
}
