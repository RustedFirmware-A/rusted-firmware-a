// Copyright (c) 2025, Arm Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

//! Dummy SPMD implementing for PSCI testing.

pub struct Spmd;

impl Spmd {
    pub fn handle_psci_event(&self, _psci_request: &[u64; 4]) -> u64 {
        0
    }
    pub fn handle_cold_boot(&self) {}

    pub fn handle_warm_boot(&self) {}
}

pub static SPMD: Spmd = Spmd;
