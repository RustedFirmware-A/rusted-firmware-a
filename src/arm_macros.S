/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2015-2019, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

	/* ---------------------------------------------
	 * The below utility macro prints out relevant GIC
	 * registers whenever an unhandled exception is
	 * taken in BL31 on ARM standard platforms.
	 * Expects: GICD base in x16
	 * Clobbers: x0 - x11, sp
	 * ---------------------------------------------
	 */
	.macro arm_print_gic_regs
	/* Save the LR. */
	mov x11, lr

	/* Check for SRE enable */
	mrs	x8, ICC_SRE_EL3
	tst	x8, #{ICC_SRE_SRE_BIT}
	b.eq	0f

	/* Load the icc reg list to x6 */
	adr	x6, icc_regs
	/* Load the icc regs to gp regs used by str_in_crash_buf_print */
	mrs	x8, ICC_HPPIR0_EL1
	mrs	x9, ICC_HPPIR1_EL1
	mrs	x10, ICC_CTLR_EL3
	/* Store to the crash buf and print to console */
	bl	str_in_crash_buf_print

0:
	/* Print the GICD_ISPENDR regs */
	add	x7, x16, #{GICD_ISPENDR}
	adr	x4, gicd_pend_reg
	bl	asm_print_str
1:
	sub	x4, x7, x16
	cmp	x4, #0x280
	b.eq	2f

	/* Print "0x" */
	adr	x4, prefix
	bl	asm_print_str

	/* Print offset */
	sub	x4, x7, x16
	mov	x5, #12
	bl	asm_print_hex_bits

	adr	x4, spacer
	bl	asm_print_str

	ldr	x4, [x7], #8
	bl	asm_print_hex

	adr	x4, newline
	bl	asm_print_str
	b	1b
2:

	/* Restore the LR. */
	mov lr, x11
	.endm
