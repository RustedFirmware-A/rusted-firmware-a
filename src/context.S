/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2024, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.global	prepare_el3_entry
.global	restore_gp_pmcr_pauth_regs
.global	el3_exit

/*
 * Set SCR_EL3.EA bit to enable SErrors at EL3
 */
.macro enable_serror_at_el3
	mrs     x18, scr_el3
	orr     x18, x18, #{SCR_EA_BIT}
	msr     scr_el3, x18
.endm

/*
 * Set the PSTATE bits not set when the exception was taken as
 * described in the AArch64.TakeException() pseudocode function
 * in ARM DDI 0487F.c page J1-7635 to a default value.
 */
.macro set_unset_pstate_bits
	/*
	 * Always enable Data Independent Timing (DIT) in EL3.
	 * NOTE: We assume that FEAT_DIT is present as it is mandatory from Armv8.4.
	 */
	mov	x18, #{DIT_BIT}
	msr	s3_3_c4_c2_5, x18 /* DIT */
.endm

/* ------------------------------------------------------------------
 * The following macro is used to save all the general purpose and
 * FEAT_PAuth registers.
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
	stp	x0, x1, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X0}]
	stp	x2, x3, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X2}]
	stp	x4, x5, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X4}]
	stp	x6, x7, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X6}]
	stp	x8, x9, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X8}]
	stp	x10, x11, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X10}]
	stp	x12, x13, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X12}]
	stp	x14, x15, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X14}]
	stp	x16, x17, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X16}]
	stp	x18, x19, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X18}]
	stp	x20, x21, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X20}]
	stp	x22, x23, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X22}]
	stp	x24, x25, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X24}]
	stp	x26, x27, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X26}]
	stp	x28, x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X28}]
	mrs	x18, sp_el0
	str	x18, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_SP_EL0}]

	/* PMUv3 is presumed to be always present */
	mrs	x18, pmcr_el0
	str	x18, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_PMCR_EL0}]
	/* Disable cycle counter when event counting is prohibited */
	orr	x18, x18, #{PMCR_EL0_DP_BIT}
	msr	pmcr_el0, x18
	isb

	/* Save the FEAT_PAuth keys as they are not banked by exception level */
	mrs	x20, s3_0_c2_c1_0 /* apiakeylo_el1 */
	mrs	x21, s3_0_c2_c1_1 /* apiakeyhi_el1 */
	mrs	x22, s3_0_c2_c1_2 /* apibkeylo_el1 */
	mrs	x23, s3_0_c2_c1_3 /* apibkeyhi_el1 */
	mrs	x24, s3_0_c2_c2_0 /* apdakeylo_el1 */
	mrs	x25, s3_0_c2_c2_1 /* apdakeyhi_el1 */
	mrs	x26, s3_0_c2_c2_2 /* apdbkeylo_el1 */
	mrs	x27, s3_0_c2_c2_3 /* apdbkeyhi_el1 */
	mrs	x28, s3_0_c2_c3_0 /* apgakeylo_el1 */
	mrs	x29, s3_0_c2_c3_1 /* apgakeyhi_el1 */

	stp	x20, x21, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APIAKEY_LO}] /* APIAKey = x21:x20 */
	stp	x22, x23, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APIBKEY_LO}] /* APIBKey = x23:x22 */
	stp	x24, x25, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APDAKEY_LO}] /* APDAKey = x25:x24 */
	stp	x26, x27, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APDBKEY_LO}] /* APDBKey = x27:x26 */
	stp	x28, x29, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APGAKEY_LO}] /* APGAKey = x29:x28 */

.if {ENABLE_PAUTH}
	/* tpidr_el3 contains the address of the cpu_data structure */
	mrs	x20, tpidr_el3
	/* Load the EL3 APIAKey from cpu_data */
	ldp	x21, x22, [x20, #{CPU_DATA_APIAKEY_OFFSET}]

	/* Program APIAKey with the EL3 key */
	msr	s3_0_c2_c1_0, x21 /* apiakeylo_el1 */
	msr	s3_0_c2_c1_1, x22 /* apiakeyhi_el1 */
.endif /* ENABLE_PAUTH */
.endm
/* save_gp_pmcr_pauth_regs */

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
	set_unset_pstate_bits
	ret
endfunc prepare_el3_entry

/* ------------------------------------------------------------------
 * This function restores all general purpose and FEAT_PAuth
 * registers except x30 from the CPU context.
 * x30 register must be explicitly restored by the caller.
 * ------------------------------------------------------------------
 */
func restore_gp_pmcr_pauth_regs
	/* Restore the FEAT_PAuth keys */
	ldp	x0, x1, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APIAKEY_LO}] /* x1:x0 = APIAKey */
	ldp	x2, x3, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APIBKEY_LO}] /* x3:x2 = APIBKey */
	ldp	x4, x5, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APDAKEY_LO}] /* x5:x4 = APDAKey */
	ldp	x6, x7, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APDBKEY_LO}] /* x7:x6 = APDBKey */
	ldp	x8, x9, [sp, #{CTX_PAUTH_REGS_OFFSET} + {CTX_APGAKEY_LO}] /* x9:x8 = APGAKey */

	msr	s3_0_c2_c1_0, x0 /* apiakeylo_el1 */
	msr	s3_0_c2_c1_1, x1 /* apiakeyhi_el1 */
	msr	s3_0_c2_c1_2, x2 /* apibkeylo_el1 */
	msr	s3_0_c2_c1_3, x3 /* apibkeyhi_el1 */
	msr	s3_0_c2_c2_0, x4 /* apdakeylo_el1 */
	msr	s3_0_c2_c2_1, x5 /* apdakeyhi_el1 */
	msr	s3_0_c2_c2_2, x6 /* apdbkeylo_el1 */
	msr	s3_0_c2_c2_3, x7 /* apdbkeyhi_el1 */
	msr	s3_0_c2_c3_0, x8 /* apgakeylo_el1 */
	msr	s3_0_c2_c3_1, x9 /* apgakeyhi_el1 */

	/* PMUv3 is presumed to be always present */
	ldr	x0, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_PMCR_EL0}]
	msr	pmcr_el0, x0
	ldp	x0, x1, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X0}]
	ldp	x2, x3, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X2}]
	ldp	x4, x5, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X4}]
	ldp	x6, x7, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X6}]
	ldp	x8, x9, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X8}]
	ldp	x10, x11, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X10}]
	ldp	x12, x13, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X12}]
	ldp	x14, x15, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X14}]
	ldp	x16, x17, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X16}]
	ldp	x18, x19, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X18}]
	ldp	x20, x21, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X20}]
	ldp	x22, x23, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X22}]
	ldp	x24, x25, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X24}]
	ldp	x26, x27, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X26}]
	ldr	x28, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_SP_EL0}]
	msr	sp_el0, x28
	ldp	x28, x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X28}]
	ret
endfunc restore_gp_pmcr_pauth_regs

/* -----------------------------------------------------------------
 * The below macro reads SCR_EL3 from the context structure to
 * determine the security state of the context upon ERET.
 * ------------------------------------------------------------------
 */
.macro get_security_state _ret:req, _scr_reg:req
	ubfx 	\_ret, \_scr_reg, #{SCR_NSE_SHIFT}, #1
	cmp 	\_ret, #1
	beq 	realm_state
	bfi	\_ret, \_scr_reg, #0, #1
	b 	end
realm_state:
	mov 	\_ret, #2
end:
.endm

.macro	restore_ptw_el1_sys_regs
.if {ERRATA_SPECULATIVE_AT}
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
	ldp	x28, x29, [sp, #{CTX_EL1_SYSREGS_OFFSET} + {CTX_SCTLR_EL1}]
	msr	sctlr_el1, x28
	isb
	msr	tcr_el1, x29
.endif
.endm


/* ------------------------------------------------------------------
 * This routine assumes that x0 is pointing to a valid CpuContext structure
 * from where the gp regs and other special registers can be retrieved, and that
 * x1 is pointing to a PerWorldContext structure.
 * Also, it sets SP_EL3 to the value from x0, which will be used when
 * handling the next exception to EL3.
 * ------------------------------------------------------------------
 */
func el3_exit
.if {ENABLE_ASSERTIONS}
	mrs	x17, spsel
	cmp	x17, #{MODE_SP_EL0}
	ASM_ASSERT eq, "el3_exit assumes SP_EL0 on entry", assume_sp_el0
.endif /* ENABLE_ASSERTIONS */

	/* ----------------------------------------------------------
	 * Save the current SP_EL0 i.e. the EL3 runtime stack and link register.
	 * Then switch to SP_EL3.
	 * ----------------------------------------------------------
	 */
	mov	x17, sp
	msr	spsel, #{MODE_SP_ELX}
	mov	sp, x0
	stp	x17, x30, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_RUNTIME_SP_LR}]

	/*
	 * FEAT_IESB provides an implicit error synchronization event at exception
	 * entry and exception return, so there is no need for any explicit
	 * synchronization instruction.
	 * NOTE: We assume FEAT_RAS is present as it is mandatory from Armv8.2. With
	 * FEAT_RAS present, we assume that FEAT_IESB is also present.
	 */

	/* ----------------------------------------------------------
	 * Restore MDCR_EL3, SPSR_EL3, ELR_EL3 and SCR_EL3 prior to ERET
	 * ----------------------------------------------------------
	 */
	ldr	x19, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_MDCR_EL3}]
	ldp	x16, x17, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_SPSR_EL3}]
	ldr	x18, [x1, #{PERWORLD_CONTEXT_SCR_EL3}]
	msr	mdcr_el3, x19
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
	ldr	x30, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_LR}]

	/* Clear the EL3 flag as we are exiting el3 */
	str	xzr, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_NESTED_EA_FLAG}]

	exception_return
endfunc el3_exit
