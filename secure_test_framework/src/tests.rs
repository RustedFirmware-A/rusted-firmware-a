// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases.

mod ffa_spmd;
mod interrupts;
mod psci;
mod psci_osi;
#[cfg(feature = "rme")]
mod rmi;
#[cfg(any(not(feature = "rme"), feature = "test_rmm_fail"))]
mod rmi_fail;
mod sctlr2;
mod simd;
mod smccc_arch;
mod sve;
