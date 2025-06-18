/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2016, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.globl plat_crash_print_regs

	/* ---------------------------------------------
	 * Prints out relevant GIC and CCI registers
	 * whenever an unhandled exception is taken in
	 * BL31.
	 * Clobbers: x0 - x11, x16, x17, sp
	 * ---------------------------------------------
	 */
func plat_crash_print_regs
	/* Save the lr */
	mov x11, lr
	mov_imm	x16, {GICD_BASE}
	arm_print_gic_regs
	mov lr, x11
	ret
endfunc plat_crash_print_regs
