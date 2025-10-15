// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::util::naked_asm;
use arm_gic::gicv3::{
    GicV3,
    registers::{Gicd, GicrSgi},
};
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use arm_sysregs::MpidrEl1;
use core::{fmt::Write, ptr::NonNull};
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
            MPIDR_CPU_MASK = const MpidrEl1::AFF0_MASK,
            MPIDR_CLUSTER_MASK = const MpidrEl1::AFF1_MASK,
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            PLATFORM_CPU_PER_CLUSTER_SHIFT = const PLATFORM_CPU_PER_CLUSTER_SHIFT,
        );
    }
}
