/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2015-2020, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

	.globl	plat_my_core_pos
	.globl	plat_crash_console_init
	.globl	plat_crash_console_putc
	.globl	plat_crash_console_flush
	.globl  plat_secondary_cold_boot_setup
	.globl	plat_cold_boot_handler

func plat_my_core_pos
	mrs	x0, mpidr_el1
	b	plat_calc_core_pos
endfunc plat_my_core_pos

	/* ---------------------------------------------
	 * int plat_crash_console_init(void)
	 * Function to initialize the crash console
	 * without a C Runtime to print crash report.
	 * Clobber list : x0, x1, x2
	 * ---------------------------------------------
	 */
func plat_crash_console_init
	mov_imm x0, {PLAT_QEMU_CRASH_UART_BASE}
	mov_imm x1, {PLAT_QEMU_CRASH_UART_CLK_IN_HZ}
	mov_imm x2, {PLAT_QEMU_CONSOLE_BAUDRATE}
	b	console_pl011_core_init
endfunc plat_crash_console_init

	/* ---------------------------------------------
	 * int plat_crash_console_putc(int c)
	 * Function to print a character on the crash
	 * console without a C Runtime.
	 * Clobber list : x1, x2
	 * ---------------------------------------------
	 */
func plat_crash_console_putc
	mov_imm	x1, {PLAT_QEMU_CRASH_UART_BASE}
	b	console_pl011_core_putc
endfunc plat_crash_console_putc

	/* ---------------------------------------------
	 * void plat_crash_console_flush(int c)
	 * Function to force a write of all buffered
	 * data that hasn't been output.
	 * Out : void.
	 * Clobber list : x0, x1
	 * ---------------------------------------------
	 */
func plat_crash_console_flush
	mov_imm	x0, {PLAT_QEMU_CRASH_UART_BASE}
	b	console_pl011_core_flush
endfunc plat_crash_console_flush

	/* ---------------------------------------------
	 * void plat_cold_boot_handler(void)
	 * Platform cold boot handler.
	 * ---------------------------------------------
	 */
func plat_cold_boot_handler
	ret
endfunc plat_cold_boot_handler
