// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in secure world.

use log::{error, info};

/// Runs the test with the given index.
pub fn run_test(index: u64) -> Result<(), ()> {
    info!("Running test {}", index);
    match index {
        _ => {
            error!("Requested to run unknown test {}", index);
            Err(())
        }
    }
}
