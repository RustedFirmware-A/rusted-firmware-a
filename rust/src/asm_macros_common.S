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