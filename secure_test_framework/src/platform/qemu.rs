// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use core::{arch::naked_asm, fmt::Write, ptr::NonNull};
use spin::{
    Once,
    mutex::{SpinMutex, SpinMutexGuard},
};

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: NonNull<PL011Registers> = NonNull::new(0x0900_0000 as _).unwrap();

/// The number of CPU clusters.
const CLUSTER_COUNT: usize = 1;
const PLATFORM_CPU_PER_CLUSTER_SHIFT: usize = 2;
/// The maximum number of CPUs in each cluster.
const MAX_CPUS_PER_CLUSTER: usize = 1 << PLATFORM_CPU_PER_CLUSTER_SHIFT;

const MPIDR_AFF0_MASK: u64 = 0x0000_00ff;
const MPIDR_AFF1_MASK: u64 = 0x0000_ff00;
const MPIDR_AFFINITY_BITS: usize = 8;

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

    #[unsafe(naked)]
    extern "C" fn core_position(mpidr: u64) -> usize {
        // TODO: Validate that the fields are within the range we expect.
        naked_asm!(
            "and	x1, x0, #{MPIDR_CPU_MASK}",
            "and	x0, x0, #{MPIDR_CLUSTER_MASK}",
            "add	x0, x1, x0, LSR #({MPIDR_AFFINITY_BITS} - {PLATFORM_CPU_PER_CLUSTER_SHIFT})",
            "ret",
            MPIDR_CPU_MASK = const MPIDR_AFF0_MASK,
            MPIDR_CLUSTER_MASK = const MPIDR_AFF1_MASK,
            MPIDR_AFFINITY_BITS = const MPIDR_AFFINITY_BITS,
            PLATFORM_CPU_PER_CLUSTER_SHIFT = const PLATFORM_CPU_PER_CLUSTER_SHIFT,
        );
    }
}
