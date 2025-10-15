/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2014-2023, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.if {CRASH_REPORTING}
	/*
	 * The cpu specific registers which need to be reported in a crash
	 * are reported via cpu_ops cpu_reg_dump function. After a matching
	 * cpu_ops structure entry is found, the correponding cpu_reg_dump
	 * in the cpu_ops is invoked.
	 */
	.globl	do_cpu_reg_dump
func do_cpu_reg_dump
	mov	x16, x30

	/* TODO: Add cpu_reg_dump to CpuOps */

	mov	x30, x16
	ret
endfunc do_cpu_reg_dump
.endif
