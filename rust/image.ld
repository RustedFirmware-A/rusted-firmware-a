/*
 * Copyright (c) 2023, Google LLC. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

/*
 * Code will start running at this symbol which is placed at the start of the
 * image.
 */
ENTRY(bl31_entrypoint)

MEMORY
{
	/* TODO: Configure this per platform. */
	/* ORIGIN is BL31_BASE for qemu */
	image : ORIGIN = 0xe0a0000, LENGTH = 0x10000
}

SECTIONS
{
	__ALLOC_START__ = .;
	/*
	 * Collect together the code.
	 */
	.text : ALIGN(4096) {
		__TEXT_START__ = .;
		*(.text.asm.bl31_entrypoint)
		*(.init.*)
		*(.text.*)
	} >image
	__TEXT_END__ = .;

	/*
	 * Collect together read-only data.
	 */
	.rodata : ALIGN(4096) {
		__RODATA_START__ = .;
		*(.rodata.*)
	} >image
	__RODATA_END__ = .;

	/*
	 * Collect together the read-write data including .bss at the end which
	 * will be zero'd by the entry code.
	 */
	.data : ALIGN(4096) {
		__DATA_START__ = .;
		*(.data.*)
		/*
		 * The entry point code assumes that .data is a multiple of 32
		 * bytes long.
		 */
		. = ALIGN(32);
		__DATA_END__ = .;
	} >image

	/* Everything beyond this point will not be included in the binary. */
	__ALLOC_END__ = .;

	/* The entry point code assumes that .bss is 16-byte aligned. */
	.bss : ALIGN(16)  {
		__BSS_START__ = .;
		*(.bss.*)
		*(COMMON)
		. = ALIGN(16);
		__BSS_END__ = .;
	} >image

	.stack (NOLOAD) : ALIGN(4096) {
		boot_stack_begin = .;
		. += 4 * 4096;
		. = ALIGN(4096);
		boot_stack_end = .;
	} >image

	/*
	 * Remove unused sections from the image.
	 */
	/DISCARD/ : {
		/* The image loads itself so doesn't need these sections. */
		*(.gnu.hash)
		*(.hash)
		*(.interp)
		*(.note.gnu.build-id)
		*(.got)
	}
}
