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
const PL011_BASE_ADDRESS: NonNull<PL011Registers> = NonNull::new(0x1C09_0000 as _).unwrap();
const GICD_BASE: NonNull<Gicd> = NonNull::new(0x2f00_0000 as _).unwrap();
const GICR_BASE: NonNull<GicrSgi> = NonNull::new(0x2f10_0000 as _).unwrap();

const FVP_CLUSTER_COUNT: usize = 2;
const FVP_MAX_CPUS_PER_CLUSTER: usize = 4;
const FVP_MAX_PE_PER_CPU: usize = 1;

static UART: Once<SpinMutex<Uart>> = Once::new();

pub struct Fvp;

// SAFETY: `core_position` is indeed a naked function, doesn't access any memory, only clobbers
// x0-x3, and returns a unique core index as long as `FVP_MAX_CPUS_PER_CLUSTER` and
// `FVP_MAX_PE_PER_CPU` are correct.
unsafe impl Platform for Fvp {
    const CORE_COUNT: usize = FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;

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
                Fvp::CORE_COUNT,
                false,
            )
        }
    }

    #[unsafe(naked)]
    extern "C" fn core_position(mpidr: MpidrEl1) -> usize {
        // TODO: Validate that the fields are within the range we expect.
        naked_asm!(
            // Check for MT bit in MPIDR. If not set, shift MPIDR to left to make it look as if in a
            // multi-threaded implementation.
            "tst	x0, #{MPIDR_MT_MASK}",
            "lsl	x3, x0, #{MPIDR_AFFINITY_BITS}",
            "csel	x3, x3, x0, eq",

            // Extract individual affinity fields from MPIDR.
            "ubfx	x0, x3, #{MPIDR_AFF0_SHIFT}, #{MPIDR_AFFINITY_BITS}",
            "ubfx	x1, x3, #{MPIDR_AFF1_SHIFT}, #{MPIDR_AFFINITY_BITS}",
            "ubfx	x2, x3, #{MPIDR_AFF2_SHIFT}, #{MPIDR_AFFINITY_BITS}",

            // Compute linear position.
            "mov	x3, #{FVP_MAX_CPUS_PER_CLUSTER}",
            "madd	x1, x2, x3, x1",
            "mov	x3, #{FVP_MAX_PE_PER_CPU}",
            "madd	x0, x1, x3, x0",
            "ret",
            MPIDR_MT_MASK = const MpidrEl1::MT.bits(),
            MPIDR_AFF0_SHIFT = const MpidrEl1::AFF0_SHIFT,
            MPIDR_AFF1_SHIFT = const MpidrEl1::AFF1_SHIFT,
            MPIDR_AFF2_SHIFT = const MpidrEl1::AFF2_SHIFT,
            FVP_MAX_CPUS_PER_CLUSTER = const FVP_MAX_CPUS_PER_CLUSTER,
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            FVP_MAX_PE_PER_CPU = const FVP_MAX_PE_PER_CPU,
        );
    }
}
