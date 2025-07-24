// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! PL011 crash console driver.

use crate::debug::{DEBUG, ENABLE_ASSERTIONS};
use core::arch::global_asm;

/// Enable FIFOs.
const PL011_UARTLCR_H_FEN: u16 = 1 << 4;
const PL011_UARTLCR_H_WLEN_8: u16 = 3 << 5;

global_asm!(
    include_str!("../asm_macros_common.S"),
    include_str!("pl011_console.S"),
    include_str!("../asm_macros_common_purge.S"),
    DEBUG = const DEBUG as i32,
    ENABLE_ASSERTIONS = const ENABLE_ASSERTIONS as u32,
    UARTDR = const 0x000,
    UARTECR = const 0x004,
    UARTFR = const 0x018,
    UARTIBRD = const 0x024,
    UARTFBRD = const 0x028,
    UARTLCR_H = const 0x02C,
    UARTCR = const 0x030,
    PL011_UARTCR_UARTEN = const 1 << 0,
    PL011_UARTCR_TXE = const 1 << 8,
    PL011_UARTCR_RXE = const 1 << 9,
    PL011_UARTFR_TXFF_BIT = const 5,
    PL011_UARTFR_BUSY_BIT = const 3,
    PL011_LINE_CONTROL = const PL011_UARTLCR_H_FEN | PL011_UARTLCR_H_WLEN_8,
    PL011_GENERIC_UART = const 0,
);
