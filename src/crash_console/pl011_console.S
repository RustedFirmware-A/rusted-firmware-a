/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2020, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

/*
 * "core" functions are low-level implementations that don't require
 * writable memory and are thus safe to call in BL1 crash context.
 */
.globl console_pl011_core_init
.globl console_pl011_core_putc
.globl console_pl011_core_flush

/* -----------------------------------------------
 * int console_pl011_core_init(uintptr_t base_addr,
 * unsigned int uart_clk, unsigned int baud_rate)
 * Function to initialize the console without a
 * C Runtime to print debug information. This
 * function will be accessed by console_init and
 * crash reporting.
 * In: x0 - console base address
 *     w1 - Uart clock in Hz
 *     w2 - Baud rate
 * Out: return 1 on success else 0 on error
 * Clobber list : x1, x2, x3, x4
 * -----------------------------------------------
 */
func console_pl011_core_init
	/* Check the input base address */
	cbz	x0, core_init_fail
.if !{PL011_GENERIC_UART}
	/* Check baud rate and uart clock for sanity */
	cbz	w1, core_init_fail
	cbz	w2, core_init_fail
	/* Disable uart before programming */
	ldr	w3, [x0, #{UARTCR}]
	mov	w4, #{PL011_UARTCR_UARTEN}
	bic	w3, w3, w4
	str	w3, [x0, #{UARTCR}]
	/* Program the baudrate */
	/* Divisor =  (Uart clock * 4) / baudrate */
	lsl	w1, w1, #2
	udiv	w2, w1, w2
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
.endif
	mov	w0, #1
	ret
core_init_fail:
	mov	w0, wzr
	ret
endfunc console_pl011_core_init

/* --------------------------------------------------------
 * int console_pl011_core_putc(int c, uintptr_t base_addr)
 * Function to output a character over the console. It
 * returns the character printed on success or -1 on error.
 * In : w0 - character to be printed
 *      x1 - console base address
 * Out : return -1 on error else return character.
 * Clobber list : x2
 * --------------------------------------------------------
 */
func console_pl011_core_putc
.if {ENABLE_ASSERTIONS}
	cmp	x1, #0
	ASM_ASSERT ne, "console_pl011_core_putc needs non-null base address", pl011_putc_nonnull
.endif /* ENABLE_ASSERTIONS */

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
	ret
endfunc console_pl011_core_putc

/* ---------------------------------------------
 * void console_pl011_core_flush(uintptr_t base_addr)
 * Function to force a write of all buffered
 * data that hasn't been output.
 * In : x0 - console base address
 * Out : void.
 * Clobber list : x0, x1
 * ---------------------------------------------
 */
func console_pl011_core_flush
.if {ENABLE_ASSERTIONS}
	cmp	x0, #0
	ASM_ASSERT ne, "console_pl011_core_flush needs non-null base address", pl011_flush_nonnull
.endif /* ENABLE_ASSERTIONS */
1:
	/* Loop until the transmit FIFO is empty */
	ldr	w1, [x0, #{UARTFR}]
	tbnz	w1, #{PL011_UARTFR_BUSY_BIT}, 1b
	ret
endfunc console_pl011_core_flush
