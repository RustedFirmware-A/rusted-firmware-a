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

