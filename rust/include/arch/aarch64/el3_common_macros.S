/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2015-2024, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#ifndef EL3_COMMON_MACROS_S
#define EL3_COMMON_MACROS_S

#include <arch.h>
#include <asm_macros.S>
#include <assert_macros.S>
#include <context.h>
#include <lib/xlat_tables/xlat_tables_defs.h>

	.macro	apply_at_speculative_wa
#if ERRATA_SPECULATIVE_AT
	/*
	 * This function expects x30 has been saved.
	 * Also, save x29 which will be used in the called function.
	 */
	str	x29, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X29]
	bl	save_and_update_ptw_el1_sys_regs
	ldr	x29, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X29]
#endif
	.endm

	.macro	restore_ptw_el1_sys_regs
#if ERRATA_SPECULATIVE_AT
	/* -----------------------------------------------------------
	 * In case of ERRATA_SPECULATIVE_AT, must follow below order
	 * to ensure that page table walk is not enabled until
	 * restoration of all EL1 system registers. TCR_EL1 register
	 * should be updated at the end which restores previous page
	 * table walk setting of stage1 i.e.(TCR_EL1.EPDx) bits. ISB
	 * ensures that CPU does below steps in order.
	 *
	 * 1. Ensure all other system registers are written before
	 *    updating SCTLR_EL1 using ISB.
	 * 2. Restore SCTLR_EL1 register.
	 * 3. Ensure SCTLR_EL1 written successfully using ISB.
	 * 4. Restore TCR_EL1 register.
	 * -----------------------------------------------------------
	 */
	isb
	ldp	x28, x29, [sp, #CTX_EL1_SYSREGS_OFFSET + CTX_SCTLR_EL1]
	msr	sctlr_el1, x28
	isb
	msr	tcr_el1, x29
#endif
	.endm

/* -----------------------------------------------------------------
 * The below macro reads SCR_EL3 from the context structure to
 * determine the security state of the context upon ERET.
 * ------------------------------------------------------------------
 */
	.macro get_security_state _ret:req, _scr_reg:req
		ubfx 	\_ret, \_scr_reg, #SCR_NSE_SHIFT, #1
		cmp 	\_ret, #1
		beq 	realm_state
		bfi	\_ret, \_scr_reg, #0, #1
		b 	end
	realm_state:
		mov 	\_ret, #2
	end:
	.endm

#endif /* EL3_COMMON_MACROS_S */
