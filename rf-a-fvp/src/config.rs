// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

/// Cluster count.
pub const FVP_CLUSTER_COUNT: usize = 2;
/// Maximum CPU count per cluster.
pub const FVP_MAX_CPUS_PER_CLUSTER: usize = 4;
/// Maximum PE count per CPU.
pub const FVP_MAX_PE_PER_CPU: usize = 1;
/// Cache writeback granule size in bytes.
pub const CACHE_WRITEBACK_GRANULE: usize = 64;
/// Core count.
pub const CORE_COUNT: usize = FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;
