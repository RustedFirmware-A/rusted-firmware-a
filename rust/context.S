/*
 * Copyright (c) 2013-2024, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include <arch.h>
#include <asm_macros.S>
#include <assert_macros.S>
#include <context.h>
#include <el3_common_macros.S>

	.global	prepare_el3_entry
	.global	restore_gp_pmcr_pauth_regs
	.global save_and_update_ptw_el1_sys_regs
	.global	el3_exit

	/*
	 * Set SCR_EL3.EA bit to enable SErrors at EL3
	 */
	.macro enable_serror_at_el3
	mrs     x8, scr_el3
	orr     x8, x8, #SCR_EA_BIT
	msr     scr_el3, x8
	.endm

/* ------------------------------------------------------------------
 * The following macro is used to save and restore all the general
 * purpose registers.
 * It also checks if the Secure Cycle Counter (PMCCNTR_EL0)
 * is disabled in EL3/Secure (ARMv8.5-PMU), wherein PMCCNTR_EL0
 * needs not to be saved/restored during world switch.
 *
 * Ideally we would only save and restore the callee saved registers
 * when a world switch occurs but that type of implementation is more
 * complex. So currently we will always save and restore these
 * registers on entry and exit of EL3.
 * clobbers: x18
 * ------------------------------------------------------------------
 */
	.macro save_gp_pmcr_pauth_regs
	stp	x0, x1, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X0]
	stp	x2, x3, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X2]
	stp	x4, x5, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X4]
	stp	x6, x7, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X6]
	stp	x8, x9, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X8]
	stp	x10, x11, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X10]
	stp	x12, x13, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X12]
	stp	x14, x15, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X14]
	stp	x16, x17, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X16]
	stp	x18, x19, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X18]
	stp	x20, x21, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X20]
	stp	x22, x23, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X22]
	stp	x24, x25, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X24]
	stp	x26, x27, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X26]
	stp	x28, x29, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X28]
	mrs	x18, sp_el0
	str	x18, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_SP_EL0]

	/* PMUv3 is presumed to be always present */
	mrs	x9, pmcr_el0
	str	x9, [sp, #CTX_EL3STATE_OFFSET + CTX_PMCR_EL0]
	/* Disable cycle counter when event counting is prohibited */
	orr	x9, x9, #PMCR_EL0_DP_BIT
	msr	pmcr_el0, x9
	isb
	.endm /* save_gp_pmcr_pauth_regs */

/* -----------------------------------------------------------------
 * This function saves the context and sets the PSTATE to a known
 * state, preparing entry to el3.
 * Save all the general purpose registers.
 * Then set any of the PSTATE bits that are not set by hardware
 * according to the Aarch64.TakeException pseudocode in the Arm
 * Architecture Reference Manual to a default value for EL3.
 * clobbers: x17
 * -----------------------------------------------------------------
 */
func prepare_el3_entry
	save_gp_pmcr_pauth_regs
	enable_serror_at_el3
	ret
endfunc prepare_el3_entry

/* ------------------------------------------------------------------
 * This function restores all general purpose registers except x30
 * from the CPU context.
 * x30 register must be explicitly restored by the caller.
 * ------------------------------------------------------------------
 */
func restore_gp_pmcr_pauth_regs
	/* PMUv3 is presumed to be always present */
	ldr	x0, [sp, #CTX_EL3STATE_OFFSET + CTX_PMCR_EL0]
	msr	pmcr_el0, x0
	ldp	x0, x1, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X0]
	ldp	x2, x3, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X2]
	ldp	x4, x5, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X4]
	ldp	x6, x7, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X6]
	ldp	x8, x9, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X8]
	ldp	x10, x11, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X10]
	ldp	x12, x13, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X12]
	ldp	x14, x15, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X14]
	ldp	x16, x17, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X16]
	ldp	x18, x19, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X18]
	ldp	x20, x21, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X20]
	ldp	x22, x23, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X22]
	ldp	x24, x25, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X24]
	ldp	x26, x27, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X26]
	ldr	x28, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_SP_EL0]
	msr	sp_el0, x28
	ldp	x28, x29, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_X28]
	ret
endfunc restore_gp_pmcr_pauth_regs

/*
 * In case of ERRATA_SPECULATIVE_AT, save SCTLR_EL1 and TCR_EL1
 * registers and update EL1 registers to disable stage1 and stage2
 * page table walk
 */
func save_and_update_ptw_el1_sys_regs
	/* ----------------------------------------------------------
	 * Save only sctlr_el1 and tcr_el1 registers
	 * ----------------------------------------------------------
	 */
	mrs	x29, sctlr_el1
	str	x29, [sp, #(CTX_EL1_SYSREGS_OFFSET + CTX_SCTLR_EL1)]
	mrs	x29, tcr_el1
	str	x29, [sp, #(CTX_EL1_SYSREGS_OFFSET + CTX_TCR_EL1)]

	/* ------------------------------------------------------------
	 * Must follow below order in order to disable page table
	 * walk for lower ELs (EL1 and EL0). First step ensures that
	 * page table walk is disabled for stage1 and second step
	 * ensures that page table walker should use TCR_EL1.EPDx
	 * bits to perform address translation. ISB ensures that CPU
	 * does these 2 steps in order.
	 *
	 * 1. Update TCR_EL1.EPDx bits to disable page table walk by
	 *    stage1.
	 * 2. Enable MMU bit to avoid identity mapping via stage2
	 *    and force TCR_EL1.EPDx to be used by the page table
	 *    walker.
	 * ------------------------------------------------------------
	 */
	orr	x29, x29, #(TCR_EPD0_BIT)
	orr	x29, x29, #(TCR_EPD1_BIT)
	msr	tcr_el1, x29
	isb
	mrs	x29, sctlr_el1
	orr	x29, x29, #SCTLR_M_BIT
	msr	sctlr_el1, x29
	isb

	ret
endfunc save_and_update_ptw_el1_sys_regs

/* -----------------------------------------------------------------
* The below macro returns the address of the per_world context for
* the security state, retrieved through "get_security_state" macro.
* The per_world context address is returned in the register argument.
* Clobbers: x9, x10
* ------------------------------------------------------------------
*/

.macro get_per_world_context _reg:req
	ldr 	x10, [sp, #CTX_EL3STATE_OFFSET + CTX_SCR_EL3]
	get_security_state x9, x10
	mov_imm	x10, (CTX_PERWORLD_EL3STATE_END - CTX_CPTR_EL3)
	mul	x9, x9, x10
	adrp	x10, per_world_context
	add	x10, x10, :lo12:per_world_context
	add	x9, x9, x10
	mov 	\_reg, x9
.endm

/* ------------------------------------------------------------------
 * This routine assumes that the SP_EL3 is pointing to a valid
 * context structure from where the gp regs and other special
 * registers can be retrieved.
 * ------------------------------------------------------------------
 */
func el3_exit
#if ENABLE_ASSERTIONS
	/* el3_exit assumes SP_EL0 on entry */
	mrs	x17, spsel
	cmp	x17, #MODE_SP_EL0
	ASM_ASSERT(eq)
#endif /* ENABLE_ASSERTIONS */

	/* ----------------------------------------------------------
	 * Save the current SP_EL0 i.e. the EL3 runtime stack which
	 * will be used for handling the next SMC.
	 * Then switch to SP_EL3.
	 * ----------------------------------------------------------
	 */
	mov	x17, sp
	msr	spsel, #MODE_SP_ELX
	str	x17, [sp, #CTX_EL3STATE_OFFSET + CTX_RUNTIME_SP]

	/* ----------------------------------------------------------
	 * Restore CPTR_EL3.
	 * ZCR is only restored if SVE is supported and enabled.
	 * Synchronization is required before zcr_el3 is addressed.
	 * ----------------------------------------------------------
	 */

	/* The address of the per_world context is stored in x9 */
	get_per_world_context x9

	ldp	x19, x20, [x9, #CTX_CPTR_EL3]
	msr	cptr_el3, x19

	ands	x19, x19, #CPTR_EZ_BIT
	beq	sve_not_enabled

	isb
	msr	S3_6_C1_C2_0, x20 /* zcr_el3 */
sve_not_enabled:

	synchronize_errors

	/* ----------------------------------------------------------
	 * Restore SPSR_EL3, ELR_EL3 and SCR_EL3 prior to ERET
	 * ----------------------------------------------------------
	 */
	ldr	x18, [sp, #CTX_EL3STATE_OFFSET + CTX_SCR_EL3]
	ldp	x16, x17, [sp, #CTX_EL3STATE_OFFSET + CTX_SPSR_EL3]
	msr	scr_el3, x18
	msr	spsr_el3, x16
	msr	elr_el3, x17

	restore_ptw_el1_sys_regs

	/* ----------------------------------------------------------
	 * Restore general purpose (including x30) and PMCR_EL0.
	 * Exit EL3 via ERET to a lower exception level.
 	 * ----------------------------------------------------------
 	 */
	bl	restore_gp_pmcr_pauth_regs
	ldr	x30, [sp, #CTX_GPREGS_OFFSET + CTX_GPREG_LR]

	/* Clear the EL3 flag as we are exiting el3 */
	str	xzr, [sp, #CTX_EL3STATE_OFFSET + CTX_NESTED_EA_FLAG]

	exception_return

endfunc el3_exit
