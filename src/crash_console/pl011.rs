// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! PL011 crash console driver.

use crate::{crash_console::CrashConsole, debug::DEBUG, naked_asm};

/// Enable FIFOs.
const PL011_UARTLCR_H_FEN: u16 = 1 << 4;
const PL011_UARTLCR_H_WLEN_8: u16 = 3 << 5;

/// PL011 UART based crash console implementation.
pub struct Pl011CrashConsole<const UART_BASE: usize, const UART_CLK: u32, const UART_BAUDRATE: u32>;

/// SAFETY: The required functions are naked functions that don't use the stack and only clobber the
/// specified registers.
unsafe impl<const UART_BASE: usize, const UART_CLK: u32, const UART_BAUDRATE: u32> CrashConsole
    for Pl011CrashConsole<UART_BASE, UART_CLK, UART_BAUDRATE>
{
    #[unsafe(naked)]
    extern "C" fn crash_console_init() -> u32 {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x0, {UART_BASE}
            mov_imm	x2, {UART_DIVISOR}
            /* Disable uart before programming */
            ldr	w3, [x0, #{UARTCR}]
            mov	w4, #{PL011_UARTCR_UARTEN}
            bic	w3, w3, w4
            str	w3, [x0, #{UARTCR}]
            /* Program the baudrate */
            /* IBRD = Divisor >> 6 */
            lsr	w1, w2, #6
            /* Write the IBRD */
            str	w1, [x0, #{UARTIBRD}]
            /* FBRD = Divisor & 0x3F */
            and	w1, w2, #0x3f
            /* Write the FBRD */
            str	w1, [x0, #{UARTFBRD}]
            mov	w1, #{PL011_LINE_CONTROL}
            str	w1, [x0, #{UARTLCR_H}]
            /* Clear any pending errors */
            str	wzr, [x0, #{UARTECR}]
            /* Enable tx, rx, and uart overall */
            mov	w1, #({PL011_UARTCR_RXE} | {PL011_UARTCR_TXE} | {PL011_UARTCR_UARTEN})
            str	w1, [x0, #{UARTCR}]
            mov	w0, #1
            ret",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            UART_BASE = const {
                assert!(UART_BASE != 0);
                UART_BASE
            },
            UART_DIVISOR = const {
                assert!(UART_CLK != 0);
                assert!(UART_BAUDRATE != 0);

                UART_CLK * 4 / UART_BAUDRATE
            },
            UARTECR = const 0x004,
            UARTIBRD = const 0x024,
            UARTFBRD = const 0x028,
            UARTLCR_H = const 0x02C,
            UARTCR = const 0x030,
            PL011_UARTCR_UARTEN = const 1 << 0,
            PL011_UARTCR_TXE = const 1 << 8,
            PL011_UARTCR_RXE = const 1 << 9,
            PL011_LINE_CONTROL = const PL011_UARTLCR_H_FEN | PL011_UARTLCR_H_WLEN_8,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_putc(char: u32) -> i32 {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x1, {UART_BASE}
            /* Prepend '\r' to '\n' */
            cmp	w0, #0xA
            b.ne	2f
        1:
            /* Check if the transmit FIFO is full */
            ldr	w2, [x1, #{UARTFR}]
            tbnz	w2, #{PL011_UARTFR_TXFF_BIT}, 1b
            mov	w2, #0xD
            str	w2, [x1, #{UARTDR}]
        2:
            /* Check if the transmit FIFO is full */
            ldr	w2, [x1, #{UARTFR}]
            tbnz	w2, #{PL011_UARTFR_TXFF_BIT}, 2b
            str	w0, [x1, #{UARTDR}]
            ret",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            UART_BASE = const {
                assert!(UART_BASE != 0);
                UART_BASE
            },
            UARTDR = const 0x000,
            UARTFR = const 0x018,
            PL011_UARTFR_TXFF_BIT = const 5,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_flush() {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x0, {UART_BASE}
        1:
            /* Loop until the transmit FIFO is empty */
            ldr	w1, [x0, #{UARTFR}]
            tbnz	w1, #{PL011_UARTFR_BUSY_BIT}, 1b
            ret",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            UART_BASE = const {
                assert!(UART_BASE != 0);
                UART_BASE
            },
            UARTFR = const 0x018,
            PL011_UARTFR_BUSY_BIT = const 3,
        );
    }
}
