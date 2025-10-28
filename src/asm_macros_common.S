/*
 * Copyright The Rusted Firmware-A Contributors.
 * Copyright (c) 2013-2019, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

/*
 * This file defines macros that must be purged in asm_macros_common_purge.S.
 */

/*
 * This macro is used to create a function label and place the
 * code into a separate text section based on the function name
 * to enable elimination of unused code during linking. It also adds
 * basic debug information to enable call stack printing most of the
 * time. The optional _align parameter can be used to force a
 * non-standard alignment (indicated in powers of 2). The default is
 * _align=2 because both Aarch32 and Aarch64 instructions must be
 * word aligned. Do *not* try to use a raw .align directive. Since func
 * switches to a new section, this would not have the desired effect.
 */
.macro func _name, _align=2
/*
 * Add Call Frame Information entry in the .debug_frame section for
 * debugger consumption. This enables callstack printing in debuggers.
 * This does not use any space in the final loaded binary, only in the
 * ELF file.
 * Note that a function manipulating the CFA pointer location (i.e. the
 * x29 frame pointer on AArch64) should declare it using the
 * appropriate .cfi* directives, or be prepared to have a degraded
 * debugging experience.
 */
.cfi_sections .debug_frame
.section .text.asm.\_name, "ax"
.type \_name, %function
/*
 * .cfi_startproc and .cfi_endproc are needed to output entries in
 * .debug_frame
 */
.cfi_startproc
.align \_align
\_name:
	/* Make function callable through function pointer with BTI */
	bti c
.endm

/*
 * This macro is used to mark the end of a function.
 */
.macro endfunc _name
.cfi_endproc
.size \_name, . - \_name
.endm

/*
 * Macro to mark instances where we're jumping to a function and don't
 * expect a return. To provide the function being jumped to with
 * additional information, we use 'bl' instruction to jump rather than
 * 'b'.
 *
 * Debuggers infer the location of a call from where LR points to, which
 * is usually the instruction after 'bl'. If this macro expansion
 * happens to be the last location in a function, that'll cause the LR
 * to point a location beyond the function, thereby misleading debugger
 * back trace. We therefore insert a 'nop' after the function call for
 * debug builds, unless 'skip_nop' parameter is non-zero.
 */
.macro no_ret _func:req, skip_nop=0
	bl \_func
	.if {DEBUG} && \skip_nop == 0
		nop
	.endif
.endm

	.macro	dcache_line_size  reg, tmp
	mrs	\tmp, ctr_el0
	ubfx	\tmp, \tmp, #16, #4
	mov	\reg, #4
	lsl	\reg, \reg, \tmp
	.endm

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

	/*
	 * Helper macro to generate the best mov/movk combinations according
	 * the value to be moved. The 16 bits from '_shift' are tested and
	 * if not zero, they are moved into '_reg' without affecting
	 * other bits.
	 */
	.macro _mov_imm16 _reg, _val, _shift
		.if (\_val >> \_shift) & 0xffff
			.if (\_val & (1 << \_shift - 1))
				movk	\_reg, (\_val >> \_shift) & 0xffff, LSL \_shift
			.else
				mov	\_reg, \_val & (0xffff << \_shift)
			.endif
		.endif
	.endm

	/*
	 * Helper macro to load arbitrary values into 32 or 64-bit registers
	 * which generates the best mov/movk combinations. Many base addresses
	 * are 64KB aligned the macro will eliminate updating bits 15:0 in
	 * that case
	 */
	.macro mov_imm _reg, _val
		.if (\_val) == 0
			mov	\_reg, #0
		.else
			_mov_imm16	\_reg, (\_val), 0
			_mov_imm16	\_reg, (\_val), 16
			_mov_imm16	\_reg, (\_val), 32
			_mov_imm16	\_reg, (\_val), 48
		.endif
	.endm

/* TODO: Set this to 1. As of Arm v8.5, it is mandatory. */
.set ENABLE_FEAT_SB, 0

	/*
	 * Macro for mitigating against speculative execution beyond ERET. Uses the
	 * speculation barrier instruction introduced by FEAT_SB, if it's enabled.
	 */
	.macro exception_return
	eret
.if ENABLE_FEAT_SB
	sb_barrier_insn
.else
	dsb	nsh
	isb
.endif
	.endm

	/*
	 * Helper macro to instruction adr <reg>, <symbol> where <symbol> is
	 * within the range +/- 4 GB.
	 */
	.macro adr_l, dst, sym
	adrp	\dst, \sym
	add	\dst, \dst, :lo12:\sym
	.endm

	/*
	 * Assembler macro to enable asm_assert. Use this macro wherever
	 * assert is required in assembly. Please note that the macro makes
	 * use of label '300' to provide the logic and the caller
	 * should make sure that this label is not used to branch prior
	 * to calling this macro.
	 */
.macro ASM_ASSERT _cc:req, message:req, message_name=assert_message
	.pushsection .rodata.str1.1, "aS"
	.L_\message_name:
			.string	"\message"
	.popsection
	b.\_cc	300f
	adr	x0, .L_\message_name
	b	asm_assert
300:
.endm
