/*
 * Copyright The Rusted Firmware-A Contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

MEMORY
{
	image : ORIGIN = 0xfdc00000, LENGTH = 0x2000000
}

ENTRY(entrypoint)

SECTIONS
{
	/*
	 * Collect together the code.
	 */
	.init : ALIGN(4096) {
		text_begin = .;
		*(.init.entrypoint)
		*(.init.*)
	} >image
	.text : {
		*(.text.*)
	} >image
	text_end = .;

	/*
	 * Collect together read-only data.
	 */
	.rodata : ALIGN(4096) {
		rodata_begin = .;
		*(.rodata.*)
	} >image
	.got : {
		*(.got)
	} >image
	rodata_end = .;

	/*
	 * Collect together the read-write data including .bss at the end which
	 * will be zero'd by the entry code.
	 */
	.data : ALIGN(4096) {
		data_begin = .;
		*(.data.*)
		/*
		 * The entry point code assumes that .data is a multiple of 32
		 * bytes long.
		 */
		. = ALIGN(32);
		data_end = .;
	} >image

	/* Everything beyond this point will not be included in the binary. */
	bin_end = .;

	/* The entry point code assumes that .bss is 16-byte aligned. */
	.bss : ALIGN(16)  {
		bss_begin = .;
		*(.bss.*)
		*(COMMON)
		. = ALIGN(16);
		bss_end = .;
	} >image

	.stack (NOLOAD) : ALIGN(4096) {
		boot_stack_begin = .;
		KEEP(*(.stack.boot_stack))
		. = ALIGN(4096);
		boot_stack_end = .;

		KEEP(*(.stack.*))
	} >image

	. = ALIGN(4K);
	PROVIDE(dma_region = .);

	/*
	 * Remove unused sections from the image.
	 */
	/DISCARD/ : {
		/* The image loads itself so doesn't need these sections. */
		*(.gnu.hash)
		*(.hash)
		*(.interp)
		*(.eh_frame_hdr)
		*(.eh_frame)
		*(.note.gnu.build-id)
	}
}
