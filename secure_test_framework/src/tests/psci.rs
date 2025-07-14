// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for PSCI calls.

use crate::{expect_eq, normal_world_test, secure_world_test};
use smccc::{Smc, psci};

normal_world_test!(test_psci_version);
fn test_psci_version() -> Result<(), ()> {
    expect_eq!(
        psci::version::<Smc>(),
        Ok(psci::Version { major: 1, minor: 3 })
    );
    Ok(())
}

secure_world_test!(test_psci_version_secure);
fn test_psci_version_secure() -> Result<(), ()> {
    expect_eq!(psci::version::<Smc>(), Err(psci::Error::NotSupported));
    Ok(())
}
