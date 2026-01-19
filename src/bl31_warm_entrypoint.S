/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2021, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

	adr     x0, runtime_exceptions
	msr     vbar_el3, x0
	isb

	bl      {apply_reset_errata}
	bl      {cpu_reset_handler}

	/* ---------------------------------------------------------------------
	 * SCTLR_EL3 has already been initialised - read current value before
	 * modifying.
	 *
	 * SCTLR_EL3.IESB: Enable implicit error synchronization events. Assume
	 *  that FEAT_IESB is present.
	 *
	 * SCTLR_EL3.I: Enable the instruction cache.
	 *
	 * SCTLR_EL3.SA: Enable Stack Alignment check. A SP alignment fault
	 *  exception is generated if a load or store instruction executed at
	 *  EL3 uses the SP as the base address and the SP is not aligned to a
	 *  16-byte boundary.
	 *
	 * SCTLR_EL3.A: Enable Alignment fault checking. All instructions that
	 *  load or store one or more registers have an alignment check that the
	 *  address being accessed is aligned to the size of the data element(s)
	 *  being accessed.
	 * ---------------------------------------------------------------------
	 */
	mov_imm x1, ({SCTLR_IESB_BIT} | {SCTLR_I_BIT} | {SCTLR_A_BIT} | {SCTLR_SA_BIT})
	mrs     x0, sctlr_el3
	orr     x0, x0, x1
	msr     sctlr_el3, x0
	isb

	/* Initialise TPIDR_EL3 to point to the current CPU's CpuData instance. */
	bl init_cpu_data_ptr

	/* ---------------------------------------------------------------------
	 * Enable External Aborts and SError Interrupts now that the exception
	 * vectors have been setup.
	 * ---------------------------------------------------------------------
	 */
	msr	daifclr, #{DAIF_ABT_BIT}

	/*
	 * Always enable Data Independent Timing (DIT) in EL3.
	 * NOTE: We assume that FEAT_DIT is present as it is mandatory from Armv8.4.
	 */
	mov     x0, #{DIT_BIT}
	msr     s3_3_c4_c2_5, x0 /* DIT */

	/* ---------------------------------------------------------------------
	 * Use SP_EL0 for the C runtime stack.
	 * ---------------------------------------------------------------------
	 */
	msr	spsel, #0

	/* ---------------------------------------------------------------------
	 * Allocate a stack whose memory will be marked as Normal-IS-WBWA when
	 * the MMU is enabled.
	 * ---------------------------------------------------------------------
	 */
	bl	{plat_set_my_stack}

	/* Enable page tables using the runtime page tables created by the primary core. */
	adr_l x0, {PAGE_TABLE_ADDR}
	ldr x0, [x0]
	mov_imm x1, ({SCTLR_M_BIT} | {SCTLR_C_BIT} | {SCTLR_WXN_BIT})
	mov x2, xzr
	bl {enable_mmu}

	// TODO: gpt_enable for RME to set up sysregs?

	b  {psci_warmboot_entrypoint}
