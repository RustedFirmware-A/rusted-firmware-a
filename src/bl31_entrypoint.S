/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2021, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.global bl31_entrypoint
.global bl31_warm_entrypoint

/* -----------------------------------------------------
 * bl31_entrypoint() is the cold boot entrypoint,
 * executed only by the primary cpu.
 * -----------------------------------------------------
 */
func bl31_entrypoint
	/* ---------------------------------------------------------------------
	 * Stash the previous bootloader arguments x0 - x3 for later use.
	 * ---------------------------------------------------------------------
	 */
	mov     x20, x0
	mov     x21, x1
	mov     x22, x2
	mov     x23, x3

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
	msr     daifclr, #{DAIF_ABT_BIT}


	/*
	 * Always enable Data Independent Timing (DIT) in EL3.
	 * NOTE: We assume that FEAT_DIT is present as it is mandatory from Armv8.4.
	 */
	mov     x0, #{DIT_BIT}
	msr     s3_3_c4_c2_5, x0 /* DIT */

	/*
	 * Invalidate cache for the entire BL31 image. Since there can be memory
	 * overlap with previous boot loader stages, without this we can end up
	 * re-using dirty cache lines from previous stages.
	 */
	adr_l    x0, __TEXT_START__
	adr_l    x1, __BL31_END__
	sub     x1, x1, x0
	bl      inv_dcache_range

	adr_l    x0, __BSS_START__

	adr_l    x1, __BSS_END__
	sub     x1, x1, x0
	bl      zeromem

	/* Invalidate cache for and zero BSS2. */
	adr_l    x0, __BSS2_START__
	adr_l    x1, __BSS2_END__
	sub     x1, x1, x0
	bl      inv_dcache_range

	adr_l    x0, __BSS2_START__
	adr_l    x1, __BSS2_END__
	sub     x1, x1, x0
	bl      zeromem

	bl      {plat_cold_boot_handler}

	/* ---------------------------------------------------------------------
	 * Use SP_EL0 for the C runtime stack.
	 * ---------------------------------------------------------------------
	 */
	msr     spsel, #0

	/* ---------------------------------------------------------------------
	 * Allocate a stack whose memory will be marked as Normal-IS-WBWA when
	 * the MMU is enabled.
	 * ---------------------------------------------------------------------
	 */
	bl	plat_set_my_stack

	/* Clear and build early page tables. */
	adr_l   x0, early_page_table_start
	adr_l     x1, early_page_table_end
	sub     x1, x1, x0
	bl      zeromem

	bl {init_early_page_tables}

	/* Enable MMU using early page tables. This maps BL31 as RWX, thus WXN is cleared. */
	adr_l x0, early_page_table_start
	mov_imm x1, ({SCTLR_M_BIT} | {SCTLR_C_BIT})
	mov x2, {SCTLR_WXN_BIT}
	bl {enable_mmu}

	mov     x0, x20
	mov     x1, x21
	mov     x2, x22
	mov     x3, x23

	b       {bl31_main}
endfunc bl31_entrypoint

/* --------------------------------------------------------------------
 * This CPU has been physically powered up. It is either resuming from
 * suspend or has simply been turned on. In both cases, call the BL31
 * warmboot entrypoint
 * --------------------------------------------------------------------
 */
func bl31_warm_entrypoint
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
	bl	plat_set_my_stack

	/* Enable page tables using the runtime page tables created by the primary core. */
	adr_l x0, {PAGE_TABLE_ADDR}
	ldr x0, [x0]
	mov_imm x1, ({SCTLR_M_BIT} | {SCTLR_C_BIT} | {SCTLR_WXN_BIT})
	mov x2, xzr
	bl {enable_mmu}

	// TODO: gpt_enable for RME to set up sysregs?

	b  {psci_warmboot_entrypoint}
endfunc bl31_warm_entrypoint
