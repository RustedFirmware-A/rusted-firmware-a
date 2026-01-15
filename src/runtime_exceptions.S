/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2024, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

	.globl	runtime_exceptions

	.globl	sync_exception_sp_el0
	.globl	irq_sp_el0
	.globl	fiq_sp_el0
	.globl	serror_sp_el0

	.globl	sync_exception_sp_elx
	.globl	irq_sp_elx
	.globl	fiq_sp_elx
	.globl	serror_sp_elx

	.globl	sync_exception_aarch64
	.globl	irq_aarch64
	.globl	fiq_aarch64
	.globl	serror_aarch64

	.globl	sync_exception_aarch32
	.globl	irq_aarch32
	.globl	fiq_aarch32
	.globl	serror_aarch32

	.set DAIF_ABT_BIT, (1 << 2)
	.set ESR_EC_SHIFT, 26
	.set ESR_EC_LENGTH, 6
	.set EC_AARCH32_SMC, 0x13
	.set EC_AARCH64_SMC, 0x17
	.set EC_AARCH64_SYS, 0x18
	.set EC_IMP_DEF_EL3, 0x1f
	.set FUNCID_CC_SHIFT, 30
	.set ASYNC_EA_REPLAY_COUNTER, 100

/*
 * Declare the exception vector table, enforcing it is aligned on a
 * 2KB boundary, as required by the ARMv8 architecture.
 * Use zero bytes as the fill value to be stored in the padding bytes
 * so that it inserts illegal AArch64 instructions. This increases
 * security, robustness and potentially facilitates debugging.
 */
.macro vector_base  label, section_name=.vectors
	.section \section_name, "ax"
	.align 11, 0
	\label:
.endm

/*
 * Create an entry in the exception vector table, enforcing it is
 * aligned on a 128-byte boundary, as required by the ARMv8 architecture.
 * Use zero bytes as the fill value to be stored in the padding bytes
 * so that it inserts illegal AArch64 instructions. This increases
 * security, robustness and potentially facilitates debugging.
 */
.macro vector_entry  label, section_name=.vectors
	.cfi_sections .debug_frame
	.section \section_name, "ax"
	.align 7, 0
	.type \label, %function
	.cfi_startproc
	\label:
.endm

/*
 * Add the bytes until fill the full exception vector, whose size is always
 * 32 instructions. If there are more than 32 instructions in the
 * exception vector then an error is emitted.
 */
.macro end_vector_entry label
	.cfi_endproc
	.fill	\label + (32 * 4) - .
.endm

	.macro	apply_at_speculative_wa
.if {ERRATA_SPECULATIVE_AT}
	/*
	 * This function expects x30 has been saved.
	 * Also, save x29 which will be used in the called function.
	 */
	str	x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X29}]
	bl	save_and_update_ptw_el1_sys_regs
	ldr	x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X29}]
.endif
	.endm

	/*
	 * Macro to unmask External Aborts by changing PSTATE.A bit.
	 * Put explicit synchronization event to ensure newly unmasked interrupt
	 * is taken immediately.
	 */
	.macro  unmask_async_ea
	msr     daifclr, #DAIF_ABT_BIT
	isb
	.endm

	/*
	 * Save LR and make x30 available as most of the routines in vector entry
	 * need a free register
	 */
	.macro save_x30
	str	x30, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_LR}]
	.endm

	.macro restore_x30
	ldr	x30, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_LR}]
	.endm

	/*
	 * Macro that synchronizes errors (EA) and checks for pending SError.
	 * On detecting a pending SError it reflects it back to lower EL (KFH).
	 */
	.macro	sync_and_handle_pending_serror
	/*
	 * FEAT_IESB provides an implicit error synchronization event at exception
	 * entry and exception return, so there is no need for any explicit
	 * synchronization instruction.
	 * NOTE: We assume FEAT_RAS is present as it is mandatory from Armv8.2. With
	 * FEAT_RAS present, we assume that FEAT_IESB is also present.
	 */
	mrs	x30, ISR_EL1
	tbz	x30, #{ISR_A_SHIFT}, 2f
1:
	/* This function never returns, but need LR for decision making */
	bl	reflect_pending_async_ea_to_lower_el
2:
	.endm

	/* ---------------------------------------------------------------------
	 * This macro handles Synchronous exceptions.
	 * Only SMC exceptions are supported.
	 * ---------------------------------------------------------------------
	 */
	.macro	handle_sync_exception

	mrs	x30, esr_el3
	ubfx	x30, x30, #ESR_EC_SHIFT, #ESR_EC_LENGTH

	/* Handle SMC exceptions separately from other synchronous exceptions */
	cmp	x30, #EC_AARCH32_SMC
	b.eq	smc_handler32

	cmp	x30, #EC_AARCH64_SMC
	b.eq	sync_handler64

	cmp	x30, #EC_AARCH64_SYS
	b.eq	sync_handler64

	cmp	x30, #EC_IMP_DEF_EL3
	b.eq	imp_def_el3_handler

1:
	/* Synchronous exceptions other than the above are unhandled */
	b	report_unhandled_exception
	.endm

vector_base runtime_exceptions

	/* ---------------------------------------------------------------------
	 * Current EL with SP_EL0 : 0x0 - 0x200
	 * ---------------------------------------------------------------------
	 */
vector_entry sync_exception_sp_el0
	/* We don't expect any synchronous exceptions from EL3 */
	b	report_unhandled_exception
end_vector_entry sync_exception_sp_el0

vector_entry irq_sp_el0
	/*
	 * EL3 code is non-reentrant. Any asynchronous exception is a serious
	 * error. Loop infinitely.
	 */
	b	report_unhandled_interrupt
end_vector_entry irq_sp_el0


vector_entry fiq_sp_el0
	b	report_unhandled_interrupt
end_vector_entry fiq_sp_el0


vector_entry serror_sp_el0
	no_ret	report_unhandled_exception
end_vector_entry serror_sp_el0

	/* ---------------------------------------------------------------------
	 * Current EL with SP_ELx: 0x200 - 0x400
	 * ---------------------------------------------------------------------
	 */
vector_entry sync_exception_sp_elx
	/*
	 * This exception will trigger if anything went wrong during a previous
	 * exception entry or exit or while handling an earlier unexpected
	 * synchronous exception. There is a high probability that SP_EL3 is
	 * corrupted.
	 */
	b	report_unhandled_exception
end_vector_entry sync_exception_sp_elx

vector_entry irq_sp_elx
	b	report_unhandled_interrupt
end_vector_entry irq_sp_elx

vector_entry fiq_sp_elx
	b	report_unhandled_interrupt
end_vector_entry fiq_sp_elx

vector_entry serror_sp_elx
	no_ret	report_unhandled_exception

end_vector_entry serror_sp_elx

	/* ---------------------------------------------------------------------
	 * Lower EL using AArch64 : 0x400 - 0x600
	 * ---------------------------------------------------------------------
	 */
vector_entry sync_exception_aarch64
	/*
	 * This exception vector will be the entry point for SMCs and traps
	 * that are unhandled at lower ELs most commonly. SP_EL3 should point
	 * to a valid cpu context where the general purpose and system register
	 * state can be saved.
	 */
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	handle_sync_exception
end_vector_entry sync_exception_aarch64

vector_entry irq_aarch64
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	b	handle_interrupt_exception
end_vector_entry irq_aarch64

vector_entry fiq_aarch64
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	b 	handle_interrupt_exception
end_vector_entry fiq_aarch64

	/*
	 * Need to synchronize any outstanding SError since we can get a burst of errors.
	 * So reuse the sync mechanism to catch any further errors which are pending.
	 */
vector_entry serror_aarch64
	b	report_unhandled_exception
end_vector_entry serror_aarch64

	/* ---------------------------------------------------------------------
	 * Lower EL using AArch32 : 0x600 - 0x800
	 * ---------------------------------------------------------------------
	 */
vector_entry sync_exception_aarch32
	/*
	 * This exception vector will be the entry point for SMCs and traps
	 * that are unhandled at lower ELs most commonly. SP_EL3 should point
	 * to a valid cpu context where the general purpose and system register
	 * state can be saved.
	 */
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	handle_sync_exception
end_vector_entry sync_exception_aarch32

vector_entry irq_aarch32
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	b	handle_interrupt_exception
end_vector_entry irq_aarch32

vector_entry fiq_aarch32
	save_x30
	apply_at_speculative_wa
	sync_and_handle_pending_serror
	unmask_async_ea
	b	handle_interrupt_exception
end_vector_entry fiq_aarch32

	/*
	 * Need to synchronize any outstanding SError since we can get a burst of errors.
	 * So reuse the sync mechanism to catch any further errors which are pending.
	 */
vector_entry serror_aarch32
	b	report_unhandled_exception
end_vector_entry serror_aarch32

	/* ---------------------------------------------------------------------
	 * The following code handles secure monitor calls.
	 * Depending upon the execution state from where the SMC has been
	 * invoked, it frees some general purpose registers to perform the
	 * remaining tasks. They involve finding the runtime service handler
	 * that is the target of the SMC & switching to runtime stacks (SP_EL0)
	 * before calling the handler.
	 *
	 * Note that x30 has been explicitly saved and can be used here
	 * ---------------------------------------------------------------------
	 */
func sync_exception_handler
smc_handler32:
	/* Check whether aarch32 issued an SMC64 */
	tbnz	x0, #FUNCID_CC_SHIFT, smc_prohibited

sync_handler64:
	/* NOTE: The code below must preserve x0-x17 */

	/*
	 * Save general purpose and ARMv8.3-PAuth registers (if enabled).
	 * Also save PMCR_EL0 and  set the PSTATE to a known state.
	 */
	bl	prepare_el3_entry

	/*
	 * Save SP_EL3. It points to the CpuContext structure that was set prior to the last ERET
	 * from EL3.
	 */
	mov	x28, sp

	/*
	 * Restore the saved runtime stack and the link register.
	 * They were saved in the CpuContext structure prior to the last ERET from EL3.
	 */
	ldp	x29, x30, [x28, #{CTX_EL3STATE_OFFSET} + {CTX_RUNTIME_SP_LR}]

	/* Switch to SP_EL0 */
	msr	spsel, #{MODE_SP_EL0}
	mov	sp, x29

	/*
	 * Save the SPSR_EL3 and ELR_EL3 in case there is a world
	 * switch during SMC handling.
	 * TODO: Revisit if all system registers can be saved later.
	 */
	mrs	x26, spsr_el3
	mrs	x27, elr_el3
	stp	x26, x27, [x28, #{CTX_EL3STATE_OFFSET} + {CTX_SPSR_EL3}]

	/* check for system register traps */
	mrs	x26, esr_el3
	ubfx	x27, x26, #ESR_EC_SHIFT, #ESR_EC_LENGTH
	cmp	x27, #EC_AARCH64_SYS
	b.eq	sysreg_handler64

	/* Handling an SMC, set the return value to indicate this. */
	mov	x18, #{RUN_RESULT_SMC}
	ret

sysreg_handler64:
	/* Handling a sysreg trap, set the return value to indicate this. */
	mov	x18, #{RUN_RESULT_SYSREG_TRAP}
	mov	x20, x26 /* ESR_EL3, containing syndrome information */
	ret

smc_prohibited:
	restore_ptw_el1_sys_regs
	ldp	x28, x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X28}]
	ldr	x30, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_LR}]
	mov	x0, #{SMC_UNK}
	exception_return

.if {DEBUG}
rt_svc_fw_critical_error:
	/* Switch to SP_ELx */
	msr	spsel, #{MODE_SP_ELX}
	no_ret	report_unhandled_exception
.endif
endfunc sync_exception_handler

	/* ---------------------------------------------------------------------
	 * This function handles FIQ or IRQ interrupts i.e. EL3, S-EL1 and NS
	 * interrupts.
	 *
	 * Note that x30 has been explicitly saved and can be used here
	 * ---------------------------------------------------------------------
	 */
func handle_interrupt_exception
	/*
	 * Save general purpose and ARMv8.3-PAuth registers (if enabled).
	 * Also save PMCR_EL0 and  set the PSTATE to a known state.
	 */
	bl	prepare_el3_entry

	/* Save the EL3 system registers needed to return from this exception */
	mrs	x0, spsr_el3
	mrs	x1, elr_el3
	stp	x0, x1, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_SPSR_EL3}]

	/* Switch to the runtime stack i.e. SP_EL0 and load runtime link register */
	ldp	x2, x30, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_RUNTIME_SP_LR}]
	msr	spsel, #{MODE_SP_EL0}
	mov	sp, x2

	/* Handling an interrupt, set the return value to indicate this. */
	mov	x18, #{RUN_RESULT_INTERRUPT}
	ret
endfunc handle_interrupt_exception

func imp_def_el3_handler
	/* Save GP registers */
	stp	x0, x1, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X0}]
	stp	x2, x3, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X2}]
	stp	x4, x5, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X4}]

	/* TODO: Do we need a CPU-specific exception handler? If so, look it up and call it here. */

	ldp	x0, x1, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X0}]
	ldp	x2, x3, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X2}]
	ldp	x4, x5, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X4}]
	restore_x30
	no_ret	report_unhandled_exception
endfunc imp_def_el3_handler

/*
 * Handler for async EA from lower EL synchronized at EL3 entry in KFH mode.
 *
 * This scenario may arise when there is an error (EA) in the system which is not
 * yet signaled to PE while executing in lower EL. During entry into EL3, the errors
 * are synchronized either implicitly or explicitly causing async EA to pend at EL3.
 *
 * On detecting the pending EA (via ISR_EL1.A) and if the EA routing model is
 * KFH (SCR_EL3.EA = 1) this handler reflects ther error back to lower EL.
 *
 * This function assumes x30 has been saved.
 */
func reflect_pending_async_ea_to_lower_el
	/*
	 * As the original exception was not handled we need to ensure that we return
	 * back to the instruction which caused the exception. To acheive that, eret
	 * to "elr-4" (Label "subtract_elr_el3") for SMC or simply eret otherwise
	 * (Label "skip_smc_check").
	 *
	 * LIMITATION: It could be that async EA is masked at the target exception level
	 * or the priority of async EA wrt to the EL3/secure interrupt is lower, which
	 * causes back and forth between lower EL and EL3. In case of back and forth between
	 * lower EL and EL3, we can track the loop count in "CTX_NESTED_EA_FLAG" and leverage
	 * previous ELR in "CTX_SAVED_ELR_EL3" to detect this cycle and further panic
	 * to indicate a problem here (Label "check_loop_ctr"). If we are in this cycle, loop
	 * counter retains its value but if we do a normal el3_exit this flag gets cleared.
	 * However, setting SCR_EL3.IESB = 1, should give priority to SError handling
	 * as per AArch64.TakeException pseudo code in Arm ARM.
	 *
	 * TODO: In future if EL3 gets a capability to inject a virtual SError to lower
	 * ELs, we can remove the el3_panic and handle the original exception first and
	 * inject SError to lower EL before ereting back.
	 */
	stp	x28, x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X28}]
	ldr	x29, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_SAVED_ELR_EL3}]
	mrs	x28, elr_el3
	cmp	x29, x28
	b.eq	check_loop_ctr
	str	x28, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_SAVED_ELR_EL3}]
	/* Zero the loop counter */
	str	xzr, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_NESTED_EA_FLAG}]
	b	skip_loop_ctr
check_loop_ctr:
	ldr	x29, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_NESTED_EA_FLAG}]
	add	x29, x29, #1
	str	x29, [sp, #{CTX_EL3STATE_OFFSET} + {CTX_NESTED_EA_FLAG}]
	cmp	x29, #ASYNC_EA_REPLAY_COUNTER
	b.ge	el3_panic
skip_loop_ctr:
	/*
	 * Logic to distinguish if we came from SMC or any other exception.
	 * Use offsets in vector entry to get which exception we are handling.
	 * In each vector entry of size 0x200, address "0x0-0x80" is for sync
	 * exception and "0x80-0x200" is for async exceptions.
	 * Use vector base address (vbar_el3) and exception offset (LR) to
	 * calculate whether the address we came from is any of the following
	 * "0x0-0x80", "0x200-0x280", "0x400-0x480" or "0x600-0x680"
	 */
	mrs	x29, vbar_el3
	sub	x30, x30, x29
	and	x30, x30, #0x1ff
	cmp	x30, #0x80
	b.ge	skip_smc_check
	/* Its a synchronous exception, Now check if it is SMC or not? */
	mrs	x30, esr_el3
	ubfx	x30, x30, #ESR_EC_SHIFT, #ESR_EC_LENGTH
	cmp	x30, #EC_AARCH32_SMC
	b.eq	subtract_elr_el3
	cmp	x30, #EC_AARCH64_SMC
	b.eq	subtract_elr_el3
	b	skip_smc_check
subtract_elr_el3:
	sub	x28, x28, #4
skip_smc_check:
	msr	elr_el3, x28
	ldp	x28, x29, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_X28}]
	ldr	x30, [sp, #{CTX_GPREGS_OFFSET} + {CTX_GPREG_LR}]
	exception_return
endfunc reflect_pending_async_ea_to_lower_el
