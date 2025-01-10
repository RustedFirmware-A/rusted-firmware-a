// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in secure world.

use crate::expect_eq;
use log::{error, info};
use smccc::{arch, psci, Smc};

/// Runs the test with the given index.
pub fn run_test(index: u64) -> Result<(), ()> {
    info!("Running test {}", index);
    match index {
        0 => test_smccc_arch(),
        1 => test_psci_version(),
        _ => {
            error!("Requested to run unknown test {}", index);
            Err(())
        }
    }
}

fn test_smccc_arch() -> Result<(), ()> {
    expect_eq!(
        arch::version::<Smc>(),
        Ok(arch::Version { major: 1, minor: 5 })
    );
    expect_eq!(arch::features::<Smc>(42), Err(arch::Error::NotSupported));
    Ok(())
}

fn test_psci_version() -> Result<(), ()> {
    expect_eq!(
        psci::version::<Smc>(),
        Ok(psci::Version { major: 1, minor: 3 })
    );
    Ok(())
}
