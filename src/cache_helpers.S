/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2022, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.globl	flush_dcache_range
.globl	inv_dcache_range
.globl	flush_dcache_to_popa_range
.globl	flush_dcache_to_popa_range_mte2

.macro	dcache_line_size  reg, tmp
	mrs	\tmp, ctr_el0
	ubfx	\tmp, \tmp, #16, #4
	mov	\reg, #4
	lsl	\reg, \reg, \tmp
.endm

/*
 * This macro can be used for implementing various data cache operations `op`
 */
.macro do_dcache_maintenance_by_mva op
	/* Exit early if size is zero */
	cbz	x1, exit_loop_\op
	dcache_line_size x2, x3
	add	x1, x0, x1
	sub	x3, x2, #1
	bic	x0, x0, x3
loop_\op:
	dc	\op, x0
	add	x0, x0, x2
	cmp	x0, x1
	b.lo	loop_\op
	dsb	sy
exit_loop_\op:
	ret
.endm

/* op: the hexadecimal instruction opcode for the cache operation */
.macro do_dcache_maintenance_instr op
	/* Exit early if size is zero */
	cbz	x1, exit_loop_\op
	dcache_line_size x2, x3
	sub	x3, x2, #1
	bic	x0, x0, x3
	add	x1, x1, x0
loop_\op:
	.inst	\op
	add	x0, x0, x2
	cmp	x0, x1
	b.lo	loop_\op
	dsb	osh
exit_loop_\op:
	ret
.endm

/* -------------------------------------------
 * DCache Clean+Invalidate by MVA from base
 * address till size. 'x0' = addr, 'x1' = size
 * -------------------------------------------
 */
func flush_dcache_range
	do_dcache_maintenance_by_mva civac
endfunc flush_dcache_range

/* ------------------------------------------
 * DCache Invalidate by MVA from base address
 * till size. 'x0' = addr, 'x1' = size
 * ------------------------------------------
 */
func inv_dcache_range
	do_dcache_maintenance_by_mva ivac
endfunc inv_dcache_range

/*
 * sys  #6, c7, c14, #1, x0
 * DC CIPAPA, X0
 */
.set dc_cipapa_x0, 0xd50e7e20

/*
 * sys #6, c7, c14, #3, x0
 * DC CIDGPAPA, X0
  */
.set dc_cigdpapa_x0, 0xd50e7ea0

/*
 * ------------------------------------------
 * DCache Clean+Invalidate by PA to POPA from
 * base address till size.
 * 'x0' = addr, 'x1' = size
 * ------------------------------------------
 */
func flush_dcache_to_popa_range
	/* dc cipapa, x0 */
	do_dcache_maintenance_instr dc_cipapa_x0
endfunc	flush_dcache_to_popa_range

/*
 * ------------------------------------------
 * Clean+Invalidate by PA to POPA (MTE2)
 * from base address till size.
 * 'x0' = addr, 'x1' = size
 * ------------------------------------------
 * On implementations with FEAT_MTE2, Root firmware must issue
 * DC_CIGDPAPA instead of DC_CIPAPA, in order to additionally
 * clean and invalidate Allocation Tags associated with the
 * affected locations.
 */
func flush_dcache_to_popa_range_mte2
	/* dc cigdpapa, x0 */
	do_dcache_maintenance_instr dc_cigdpapa_x0
endfunc	flush_dcache_to_popa_range_mte2

