// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    pagetable::{GRANULE_SIZE, TOP_LEVEL_BLOCK_SIZE, TOP_LEVEL_DESCRIPTOR_COUNT},
    platform::EARLY_PAGE_TABLE_RANGES,
};
use aarch64_paging::paging::Attributes;
use core::ops::Range;

/// Structure for storing early page table regions.
pub struct EarlyRegion {
    /// Physical address range.
    pub address_range: Range<usize>,
    /// Attributes of the mapped memory.
    pub attributes: Attributes,
}

/// The early page is stored as an array of `DescriptorRange`. Each item describes one or more
/// entries in the page table and stores them as (index, value) pairs. Consecutive block descriptors
/// tend to have incremental OA field. This property enables them storing more efficiently by
/// only storing the first descriptor along with the repetition count and the increment value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(C)]
pub struct DescriptorRange {
    /// Index of the descriptor in the flattened page table array.
    index: usize,
    /// This field is split into two by the granule size mask. The upper part is the step value
    /// that is added to each descriptor value. The lower part is the count of the consecutive
    /// block descriptors.
    /// It is 0 for table descriptors.
    step_count: usize,
    /// Descriptor base value. For table descriptor it contains the offset of the next level table
    /// in the page table array.
    value: usize,
}

/// Converts a list of memory regions into a list of `DescriptorRange`. Returns the generated
/// `DescriptorRange` array and the required size of the page tables in bytes after extracting them
/// from the `DescriptorRange` list.
pub const fn build_ranges<const N: usize>(
    regions: &[EarlyRegion],
) -> ([DescriptorRange; N], usize) {
    let mut ranges = [const {
        DescriptorRange {
            index: 0,
            value: 0,
            step_count: 0,
        }
    }; N];

    let (entry_count, _) = build_ranges_for_table(
        regions,
        &mut ranges,
        0,
        0,
        0,
        TOP_LEVEL_BLOCK_SIZE,
        TOP_LEVEL_DESCRIPTOR_COUNT,
    );

    (ranges, entry_count * 8)
}

/// Recursive function that builds `DescriptorRanges` for a table of the tree based on the defined
/// `regions`. It behaves like it is building actual page tables recursively but it only sets the
/// the items of `ranges`.
///
/// * regions: input memory regions and their attributes
/// * ranges: output ranges
/// * output_index: next index to be used in ranges
/// * start_entry: index of the first descriptor in the flattened page table array
/// * block_base_address: base VA of the current level page table
/// * block_size: size of the blocks in the current level page table
/// * descriptor_count: descriptor size at the current level page table
///
/// Returns the total number of the page tables descriptors and the next empty `DescriptorRange`
/// index. These values are used to move the imaginary address in the flattened page table and to
/// move the output pointer (for indexing `ranges`).
const fn build_ranges_for_table(
    regions: &[EarlyRegion],
    ranges: &mut [DescriptorRange],
    mut output_index: usize,
    start_entry: usize,
    block_base_address: usize,
    block_size: usize,
    descriptor_count: usize,
) -> (usize, usize) {
    let mut used_entry_count = GRANULE_SIZE / 8;
    let mut descriptor_index = 0;

    // Looping the descriptors of the current table.
    loop {
        if descriptor_index >= descriptor_count {
            break;
        }

        let block = (block_base_address + descriptor_index * block_size)
            ..(block_base_address + (descriptor_index + 1) * block_size);

        // Loop each address range and check if the descriptor
        let mut region_index = 0;
        loop {
            if region_index >= regions.len() {
                break;
            }

            let range = &regions[region_index].address_range;
            let attr = regions[region_index].attributes.bits();

            if range.start <= block.start && block.end <= range.end {
                // The block is fully covered by a region, insert block descriptor.

                // Calculate description repetition count.
                let repeat_count = min(
                    (range.end - block.end) / block_size,
                    descriptor_count - descriptor_index,
                );

                let lsb = if GRANULE_SIZE == block_size {
                    0b11
                } else {
                    0b01
                };

                ranges[output_index] = DescriptorRange {
                    index: (descriptor_index + start_entry) * 8,
                    step_count: block_size | (repeat_count + 1),
                    value: block.start | attr | lsb,
                };

                descriptor_index += repeat_count;
                output_index += 1;
                break;
            } else if overlaps(&block, range) {
                // There's a region that overlaps with the block but with a smaller granule, make it
                // into a table descriptor.
                let next_descriptor_count = GRANULE_SIZE / 8;
                assert!(block_size / next_descriptor_count >= GRANULE_SIZE);

                ranges[output_index] = DescriptorRange {
                    index: (descriptor_index + start_entry) * 8,
                    step_count: 0,
                    value: ((start_entry + used_entry_count) * 8) | 0b11,
                };

                // Create next level table.
                let (new_entry_count, new_output_index) = build_ranges_for_table(
                    regions,
                    ranges,
                    output_index + 1,
                    start_entry + used_entry_count,
                    block.start,
                    block_size / next_descriptor_count,
                    next_descriptor_count,
                );

                output_index = new_output_index;
                used_entry_count += new_entry_count;
                break;
            }

            region_index += 1;
        }

        descriptor_index += 1;
    }
    (used_entry_count, output_index)
}

/// Calculates the count of the necessary `DescriptorRanges` for a given region list in const time.
pub const fn get_range_count(regions: &[EarlyRegion]) -> usize {
    get_range_count_of_table(
        regions,
        0,
        TOP_LEVEL_BLOCK_SIZE,
        TOP_LEVEL_DESCRIPTOR_COUNT,
        GRANULE_SIZE,
    )
}

/// Recursive function that calculates the count of the necessary `DescriptorRanges` for a given
/// region list. It returns the descriptor count for the table that is currently processed for all
/// the referenced lower level tables. Calling this function on the top level table returns the
/// total count of the necessary `DescriptorRanges`.
const fn get_range_count_of_table(
    regions: &[EarlyRegion],
    block_base_address: usize,
    block_size: usize,
    descriptor_count: usize,
    granule_size: usize,
) -> usize {
    let mut count = 0;

    let mut descriptor_index = 0;
    loop {
        if descriptor_index >= descriptor_count {
            break;
        }

        let block = (block_base_address + descriptor_index * block_size)
            ..(block_base_address + (descriptor_index + 1) * block_size);

        let mut region_index = 0;
        loop {
            if region_index >= regions.len() {
                break;
            }

            let range = &regions[region_index].address_range;

            if range.start <= block.start && block.end <= range.end {
                // The block is fully covered by a region, insert block descriptor.

                // Calculate description repetition count.
                descriptor_index += min(
                    (range.end - block.end) / block_size,
                    descriptor_count - descriptor_index,
                );

                count += 1;
                break;
            } else if overlaps(&block, range) {
                // There's a region that overlaps with the block but with a smaller granule, make it
                // into a table descriptor.
                let next_descriptor_count = granule_size / 8;
                assert!(block_size / next_descriptor_count >= granule_size);

                count += get_range_count_of_table(
                    regions,
                    block.start,
                    block_size / next_descriptor_count,
                    next_descriptor_count,
                    granule_size,
                ) + 1;
                break;
            }

            region_index += 1;
        }

        descriptor_index += 1;
    }

    count
}

/// const version of usize::min.
const fn min(a: usize, b: usize) -> usize {
    if a < b { a } else { b }
}

/// const version of usize::max.
const fn max(a: usize, b: usize) -> usize {
    if a > b { a } else { b }
}

/// const version of Range::overlaps.
const fn overlaps(a: &Range<usize>, b: &Range<usize>) -> bool {
    max(a.start, b.start) < min(a.end, b.end)
}

macro_rules! define_early_mapping {
    ($regions:expr) => {
        const EARLY_PAGE_TABLE_RANGE_COUNT: usize =
            $crate::pagetable::early_pagetable::get_range_count(&$regions);

        const RANGES_AND_COUNT: (
            [$crate::pagetable::early_pagetable::DescriptorRange; EARLY_PAGE_TABLE_RANGE_COUNT],
            usize,
        ) = $crate::pagetable::early_pagetable::build_ranges(&$regions);

        pub static EARLY_PAGE_TABLE_RANGES: [$crate::pagetable::early_pagetable::DescriptorRange;
            EARLY_PAGE_TABLE_RANGE_COUNT] = RANGES_AND_COUNT.0;
        pub const EARLY_PAGE_TABLE_SIZE: usize = RANGES_AND_COUNT.1;
    };
}

pub(crate) use define_early_mapping;

#[cfg(all(target_arch = "aarch64", not(test)))]
/// Builds the early page tables from `EARLY_PAGE_TABLE_RANGES`.
#[unsafe(naked)]
#[unsafe(no_mangle)]
extern "C" fn init_early_page_tables() {
    use crate::naked_asm;
    use core::mem::offset_of;

    naked_asm!(
        "/* x0 = RANGES start */
        ldr	x0, ={ranges}

        /* x1 = RANGES end */
        ldr	x1, =({ranges} + ({ranges_size} * {ranges_count}))

        /* x2 = table base address */
        ldr	x2, =early_page_table_start

    1:
        /* x3 = range.index and x4 = range.step_count, x5 = range.value */
        ldp	x3, x4, [x0, #{index_step_count_offset}];
        ldr	x5, [x0, #{value_offset}]

        /* If step_count is zero, the entry is a table descriptor. */
        cbz	x4, 3f

        /* Block descriptors */

        /* x4 = step, x6 = index + (count * 8) */
        and	x6, x4, #{count_mask}
        sub	x4, x4, x6
        lsl	x6, x6, #3
        add	x6, x3, x6

    2:
        /* Block descriptor loop */

        /* *(table_base + index) = range.value */
        str	x5, [x2, x3]

        /* index += 8 */
        add	x3, x3, #8

        /* index != end_index */
        cmp	x3, x6
        b.eq	4f

        /* range.value += step */
        add	x5, x5, x4
        b	2b

    3:
        /* Table descriptor */

        /* *(table_base + index) = table_base + range.value */
        add	x5, x2, x5
        str	x5, [x2, x3]

    4:
        /* range += sizeof(DescriptorRange) */
        add	x0, x0, #{ranges_size}

        /* Check end of list */
        cmp	x0, x1
        b.ne	1b

        /* Instruction and data barrier */
        isb
        dsb	sy

        ret",
        ranges = sym EARLY_PAGE_TABLE_RANGES,
        ranges_size = const size_of::<DescriptorRange>(),
        ranges_count = const EARLY_PAGE_TABLE_RANGES.len(),
        index_step_count_offset = const offset_of!(DescriptorRange, index),
        value_offset = const offset_of!(DescriptorRange, value),
        count_mask = const (GRANULE_SIZE - 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_early_mapping() {
        assert_eq!(EARLY_PAGE_TABLE_RANGES, []);
    }

    #[test]
    fn map_empty() {
        define_early_mapping!([]);

        assert_eq!(EARLY_PAGE_TABLE_RANGES, []);
        assert_eq!(0x1000, EARLY_PAGE_TABLE_SIZE);
    }

    #[test]
    fn map_level0() {
        const REGIONS: [EarlyRegion; 1] = [EarlyRegion {
            address_range: 0x8000_0000..0xC000_0000,
            attributes: Attributes::from_bits_retain(0x10),
        }];

        define_early_mapping!(REGIONS);

        assert_eq!(
            [DescriptorRange {
                index: 0x0010,
                step_count: 0x4000_0001,
                value: 0x8000_0011,
            }],
            EARLY_PAGE_TABLE_RANGES
        );
        assert_eq!(0x1000, EARLY_PAGE_TABLE_SIZE);
    }

    #[test]
    fn map_level1() {
        const REGIONS: [EarlyRegion; 1] = [EarlyRegion {
            address_range: 0x8000_0000..0x8040_0000,
            attributes: Attributes::from_bits_retain(0x20),
        }];

        define_early_mapping!(REGIONS);

        assert_eq!(
            [
                DescriptorRange {
                    index: 0x0010,
                    step_count: 0,
                    value: 0x0000_1003,
                },
                DescriptorRange {
                    index: 0x1000,
                    step_count: 0x0020_0002,
                    value: 0x8000_0021,
                }
            ],
            EARLY_PAGE_TABLE_RANGES
        );
        assert_eq!(0x2000, EARLY_PAGE_TABLE_SIZE);
    }

    #[test]
    fn map_level2() {
        const REGIONS: [EarlyRegion; 1] = [EarlyRegion {
            address_range: 0x8000_0000..0x8000_3000,
            attributes: Attributes::from_bits_retain(0x30),
        }];

        define_early_mapping!(REGIONS);

        assert_eq!(
            [
                DescriptorRange {
                    index: 0x0010,
                    step_count: 0,
                    value: 0x0000_1003,
                },
                DescriptorRange {
                    index: 0x1000,
                    step_count: 0,
                    value: 0x0000_2003,
                },
                DescriptorRange {
                    index: 0x2000,
                    step_count: 0x0000_1003,
                    value: 0x8000_0033,
                }
            ],
            EARLY_PAGE_TABLE_RANGES
        );
        assert_eq!(0x3000, EARLY_PAGE_TABLE_SIZE);
    }

    #[test]
    fn map_complex() {
        const REGIONS: [EarlyRegion; 3] = [
            EarlyRegion {
                address_range: 0x0000_0000..0x4000_0000,
                attributes: Attributes::from_bits_retain(0x10),
            },
            EarlyRegion {
                address_range: 0x4000_0000..0x4040_0000,
                attributes: Attributes::from_bits_retain(0x20),
            },
            EarlyRegion {
                address_range: 0x8000_0000..0x8000_3000,
                attributes: Attributes::from_bits_retain(0x30),
            },
        ];

        define_early_mapping!(REGIONS);

        assert_eq!(
            [
                DescriptorRange {
                    index: 0x0000,
                    step_count: 0x4000_0001,
                    value: 0x0000_0011,
                },
                DescriptorRange {
                    index: 0x0008,
                    step_count: 0,
                    value: 0x0000_1003,
                },
                DescriptorRange {
                    index: 0x1000,
                    step_count: 0x020_0002,
                    value: 0x4000_0021,
                },
                DescriptorRange {
                    index: 0x0010,
                    step_count: 0,
                    value: 0x0000_2003,
                },
                DescriptorRange {
                    index: 0x2000,
                    step_count: 0x0000_0000,
                    value: 0x0000_3003,
                },
                DescriptorRange {
                    index: 0x3000,
                    step_count: 0x0000_1003,
                    value: 0x8000_0033,
                }
            ],
            EARLY_PAGE_TABLE_RANGES
        );
        assert_eq!(0x4000, EARLY_PAGE_TABLE_SIZE);
    }
}
