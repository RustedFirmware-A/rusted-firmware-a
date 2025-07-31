/*
 * Copyright The Rusted Firmware-A Contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

MEMORY
{
	image : ORIGIN = BL31_BASE, LENGTH = BL31_SIZE
}

/*
 * Code will start running at this symbol which is placed at the start of the
 * image.
 */
ENTRY(bl31_entrypoint)

SECTIONS
{
	__BL31_START__ = bl31_entrypoint;

	/*
	 * Collect together the code.
	 */
	.text : ALIGN(4096) {
		__TEXT_START__ = .;
		*(.text.asm.bl31_entrypoint)
		*(.init.*)
		*(.text.*)
		*(.vectors)
	} >image
	__TEXT_END__ = .;

	/*
	 * Collect together read-only data.
	 */
	.rodata : ALIGN(4096) {
		__RODATA_START__ = .;
		*(.rodata.*)
		. = ALIGN(8);
		__CPU_OPS_START__ = .;
		KEEP(*(.cpu_ops))
		__CPU_OPS_END__ = .;
	} >image
	__RODATA_END__ = .;

	. = ALIGN(4096);
	__RO_END__ = .;

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

	.stacks (NOLOAD) : ALIGN(4096) {
		__STACKS_START__ = .;
		*(.tzfw_normal_stacks)
		__STACKS_END__ = .;
		. = ALIGN(4096);
	} >image

	. = ALIGN(4096);
	__BL31_END__ = .;

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
