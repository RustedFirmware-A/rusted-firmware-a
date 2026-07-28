/*
 * Copyright The Rusted Firmware-A Contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

MEMORY
{
	image : ORIGIN = BL31_BASE, LENGTH = BL31_SIZE
	bl31_dram : ORIGIN = BL31_DRAM_BASE, LENGTH = BL31_DRAM_SIZE
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
	.text : ALIGN(PAGE_SIZE) {
		__TEXT_START__ = .;
		*(.text.bl31_entrypoint)
		*(.init.*)
		*(SORT_BY_ALIGNMENT(.text.*))
		*(.vectors)
	} >image
	__TEXT_END__ = .;

	/*
	 * Collect together read-only data.
	 */
	.rodata : ALIGN(PAGE_SIZE) {
		__RODATA_START__ = .;
		*(SORT_BY_ALIGNMENT(.rodata.*))
		. = ALIGN(8);
	} >image
	__RODATA_END__ = .;

	. = ALIGN(PAGE_SIZE);
	__RO_END__ = .;

	/*
	 * Collect together the read-write data including .bss at the end which
	 * will be zero'd by the entry code.
	 */
	.data : ALIGN(PAGE_SIZE) {
		__DATA_START__ = .;
		*(SORT_BY_ALIGNMENT(.data.*))
		/*
		 * The entry point code assumes that .data is a multiple of 32
		 * bytes long.
		 */
		. = ALIGN(32);
		__DATA_END__ = .;
	} >image

	.percore : ALIGN(CACHE_WRITEBACK_GRANULE) {
		__PERCORE_START__ = .;
		*(SORT_BY_ALIGNMENT(.percore .percore.*))

		/*
		 * Round up the section size to the actual alignment of the `.percore` section.
		 * This ensures that we can conceptually have an array of .percore sections inside the
		 * .percore_secondary section.
		 */
		. = ALIGN(ALIGNOF(.percore));
		__PERCORE_END__ = .;
	} >image

	/* Everything beyond this point will not be included in the binary. */

	.percore_secondary (NOLOAD) : ALIGN(ALIGNOF(.percore)) {
		__PERCORE_SECONDARY_START__ = .;
		. += (__PERCORE_END__ - __PERCORE_START__) * (CORE_COUNT - 1);
		__PERCORE_SECONDARY_END__ = .;
	} >image

	/* The entry point code assumes that .bss is 16-byte aligned. */
	.bss : ALIGN(16)  {
		__BSS_START__ = .;
		*(SORT_BY_ALIGNMENT(.bss.*))
		*(COMMON)
		. = ALIGN(16);
		__BSS_END__ = .;
	} >image

	.stacks (NOLOAD) : ALIGN(PAGE_SIZE) {
		__STACKS_START__ = .;
		*(.tzfw_normal_stacks)
		__STACKS_END__ = .;
		. = ALIGN(PAGE_SIZE);
	} >image

	. = ALIGN(PAGE_SIZE);
	__BL31_END__ = .;

	/* DRAM section */
	. = BL31_DRAM_BASE;

	.bss2.dram (NOLOAD) : ALIGN(PAGE_SIZE) {
		__BSS2_START__ = .;

		*(.bss2.dram)

		. = ALIGN(PAGE_SIZE);
		__BSS2_END__ = .;
	} >bl31_dram

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
