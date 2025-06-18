/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2015-2019, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.section .rodata.gic_reg_name, "aS"
/* Applicable only to GICv3 with SRE enabled */
icc_regs:
	.asciz "icc_hppir0_el1", "icc_hppir1_el1", "icc_ctlr_el3", ""

/* Registers common to both GICv2 and GICv3 */
gicd_pend_reg:
	.asciz "gicd_ispendr regs (Offsets 0x200-0x278)\nOffset\t\t\tValue\n"
newline:
	.asciz "\n"
spacer:
	.asciz ":\t\t 0x"
prefix:
	.asciz "0x"

	/* ---------------------------------------------
	 * The below utility macro prints out relevant GIC
	 * registers whenever an unhandled exception is
	 * taken in BL31 on ARM standard platforms.
	 * Expects: GICD base in x16
	 * Clobbers: x0 - x10, sp
	 * ---------------------------------------------
	 */
	.macro arm_print_gic_regs
	/* Check for SRE enable */
	mrs	x8, ICC_SRE_EL3
	tst	x8, #{ICC_SRE_SRE_BIT}
	b.eq	print_gic_common

	/* Load the icc reg list to x6 */
	adr	x6, icc_regs
	/* Load the icc regs to gp regs used by str_in_crash_buf_print */
	mrs	x8, ICC_HPPIR0_EL1
	mrs	x9, ICC_HPPIR1_EL1
	mrs	x10, ICC_CTLR_EL3
	/* Store to the crash buf and print to console */
	bl	str_in_crash_buf_print

print_gic_common:
	/* Print the GICD_ISPENDR regs */
	add	x7, x16, #{GICD_ISPENDR}
	adr	x4, gicd_pend_reg
	bl	asm_print_str
gicd_ispendr_loop:
	sub	x4, x7, x16
	cmp	x4, #0x280
	b.eq	exit_print_gic_regs

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
	b	gicd_ispendr_loop
exit_print_gic_regs:
	.endm
