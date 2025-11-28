// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    pagetable::{DEVICE_ATTRIBUTES, MEMORY_ATTRIBUTES},
    util::naked_asm,
};
use aarch64_paging::descriptor::Attributes;
use aarch64_rt::InitialPagetable;
use arm_gic::gicv3::{
    GicV3,
    registers::{Gicd, GicrSgi},
};
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use arm_sysregs::MpidrEl1;
use core::{arch::global_asm, fmt::Write, ptr::NonNull};
use spin::{
    Once,
    mutex::{SpinMutex, SpinMutexGuard},
};

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: NonNull<PL011Registers> = NonNull::new(0x0900_0000 as _).unwrap();
const GICD_BASE: NonNull<Gicd> = NonNull::new(0x0800_0000 as _).unwrap();
const GICR_BASE: NonNull<GicrSgi> = NonNull::new(0x080A_0000 as _).unwrap();

/// The number of CPU clusters.
const CLUSTER_COUNT: usize = 1;
const PLATFORM_CPU_PER_CLUSTER_SHIFT: usize = 2;
/// The maximum number of CPUs in each cluster.
const MAX_CPUS_PER_CLUSTER: usize = 1 << PLATFORM_CPU_PER_CLUSTER_SHIFT;

static UART: Once<SpinMutex<Uart>> = Once::new();

pub struct Qemu;

// SAFETY: `core_position` is indeed a naked function, doesn't access any memory, only clobbers x0
// and x1, and returns a unique core index as long as PLATFORM_CPU_PER_CLUSTER_SHIFT is correct.
unsafe impl Platform for Qemu {
    const CORE_COUNT: usize = CLUSTER_COUNT * MAX_CPUS_PER_CLUSTER;

    fn make_log_sink() -> &'static mut (dyn Write + Send) {
        let uart = UART.call_once(|| {
            // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
            // accesses that address range.
            SpinMutex::new(Uart::new(unsafe {
                UniqueMmioPointer::new(PL011_BASE_ADDRESS)
            }))
        });
        let uart: &'static mut Uart = SpinMutexGuard::leak(uart.lock());
        uart
    }

    unsafe fn create_gic() -> GicV3<'static> {
        // Safety: GICD_BASE refers exclusively to the distributor register block, with no other
        // references. Similarly, GICR_BASE refers exclusively to the redistributor register block,
        // with no other references. The caller guarantees that this function is only called once.
        unsafe {
            GicV3::new(
                UniqueMmioPointer::new(GICD_BASE),
                GICR_BASE,
                Qemu::CORE_COUNT,
                false,
            )
        }
    }

    #[unsafe(naked)]
    extern "C" fn core_position(mpidr: MpidrEl1) -> usize {
        // TODO: Validate that the fields are within the range we expect.
        naked_asm!(
            "and	x1, x0, #{MPIDR_CPU_MASK}",
            "and	x0, x0, #{MPIDR_CLUSTER_MASK}",
            "add	x0, x1, x0, LSR #({MPIDR_AFFINITY_BITS} - {PLATFORM_CPU_PER_CLUSTER_SHIFT})",
            "ret",
            MPIDR_CPU_MASK = const MpidrEl1::AFF0_MASK << MpidrEl1::AFF0_SHIFT,
            MPIDR_CLUSTER_MASK = const MpidrEl1::AFF1_MASK << MpidrEl1::AFF1_SHIFT,
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            PLATFORM_CPU_PER_CLUSTER_SHIFT = const PLATFORM_CPU_PER_CLUSTER_SHIFT,
        );
    }

    fn psci_mpidr_for_core(core_index: usize) -> u64 {
        assert!(core_index < Self::CORE_COUNT);

        let aff0 = (core_index % MAX_CPUS_PER_CLUSTER) as u64;
        let aff1 = (core_index / MAX_CPUS_PER_CLUSTER) as u64;
        aff0 | aff1 << MpidrEl1::AFFINITY_BITS
    }
}

pub static BL33_IDMAP: InitialPagetable = {
    let mut idmap = [0; 512];
    idmap[0] = DEVICE_ATTRIBUTES.bits();
    idmap[1] = MEMORY_ATTRIBUTES.bits() | 0x40000000;
    InitialPagetable(idmap)
};

global_asm!(
    "
.section \".rodata.BL32_IDMAP\", \"a\", %progbits
.global BL32_IDMAP
.align 12
BL32_IDMAP:
    .quad {TABLE_ATTRIBUTES} + 0f
    .fill 511, 8, 0x0

    /* level 2, 2 MiB block mappings */
0:
    .fill 64, 8, 0x0
    .quad {DEVICE_ATTRIBUTES} | 0x08000000
    .fill 7, 8, 0x0
    .quad {DEVICE_ATTRIBUTES} | 0x09000000
    .fill 39, 8, 0x0
    .quad {MEMORY_ATTRIBUTES} | 0x0e000000
    .fill 399, 8, 0x0
",
    DEVICE_ATTRIBUTES = const DEVICE_ATTRIBUTES.bits(),
    MEMORY_ATTRIBUTES = const MEMORY_ATTRIBUTES.bits(),
    TABLE_ATTRIBUTES = const Attributes::VALID.union(Attributes::TABLE_OR_PAGE).bits(),
);

unsafe extern "C" {
    pub static BL32_IDMAP: InitialPagetable;
}
