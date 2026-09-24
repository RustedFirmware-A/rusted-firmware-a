// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Arm platforms specific `CoreIndex` logic.

use rf_a_core::{context::CoreIndex, naked_asm, reexports::arm_sysregs::el1::registers::MpidrEl1};

/// Provides `CoreIndex` implementation for platforms that have the same number of CPUs per cluster
/// and same number of PEs per CPU.
pub struct UniformCoreIndex<
    const CLUSTER_COUNT: usize,
    const CPU_PER_CLUSTER: usize,
    const PE_PER_CPU: usize,
>;

// SAFETY: `mpidr_to_index` is indeed a naked function, doesn't access the stack or any other memory,
// only clobbers x0-x5, and returns a unique core index as long as `CPU_PER_CLUSTER` and
// `PE_PER_CPU` are correct.
unsafe impl<const CLUSTER_COUNT: usize, const CPU_PER_CLUSTER: usize, const PE_PER_CPU: usize>
    CoreIndex for UniformCoreIndex<CLUSTER_COUNT, CPU_PER_CLUSTER, PE_PER_CPU>
{
    const CORE_COUNT: usize = CLUSTER_COUNT * CPU_PER_CLUSTER * PE_PER_CPU;

    fn is_valid_mpidr(mpidr: MpidrEl1) -> bool {
        const {
            assert!(0 < CLUSTER_COUNT && CLUSTER_COUNT <= 256);
            assert!(0 < CPU_PER_CLUSTER && CPU_PER_CLUSTER <= 256);
            assert!(0 < PE_PER_CPU && PE_PER_CPU <= 256);
        }

        if mpidr.contains(MpidrEl1::MT) {
            mpidr.aff3() == 0
                && usize::from(mpidr.aff2()) < CLUSTER_COUNT
                && usize::from(mpidr.aff1()) < CPU_PER_CLUSTER
                && usize::from(mpidr.aff0()) < PE_PER_CPU
        } else {
            mpidr.aff3() == 0
                && mpidr.aff2() == 0
                && usize::from(mpidr.aff1()) < CLUSTER_COUNT
                && usize::from(mpidr.aff0()) < CPU_PER_CLUSTER
        }
    }

    /// Calculates core linear index as: ClusterId * CPU_PER_CLUSTER * PE_PER_CPU +
    /// CPUId * PE_PER_CPU + ThreadId
    #[unsafe(naked)]
    extern "C" fn mpidr_to_index(mpidr: MpidrEl1) -> usize {
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
            "mov	x4, #{CPU_PER_CLUSTER}",
            "madd	x1, x2, x4, x1",
            "mov	x5, #{PE_PER_CPU}",
            "madd	x0, x1, x5, x0",
            "ret",
            MPIDR_MT_MASK = const MpidrEl1::MT.bits(),
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            MPIDR_AFF0_SHIFT = const MpidrEl1::AFF0_SHIFT,
            MPIDR_AFF1_SHIFT = const MpidrEl1::AFF1_SHIFT,
            MPIDR_AFF2_SHIFT = const MpidrEl1::AFF2_SHIFT,
            CPU_PER_CLUSTER = const CPU_PER_CLUSTER,
            PE_PER_CPU = const PE_PER_CPU,
        );
    }
}
