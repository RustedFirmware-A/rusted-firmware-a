// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in secure world.

use crate::{expect_eq, secure_world_test};
use smccc::{Smc, arch, psci};

secure_world_test!(test_smccc_arch);
fn test_smccc_arch() -> Result<(), ()> {
    expect_eq!(
        arch::version::<Smc>(),
        Ok(arch::Version { major: 1, minor: 5 })
    );
    expect_eq!(arch::features::<Smc>(42), Err(arch::Error::NotSupported));
    Ok(())
}

secure_world_test!(test_psci_version);
fn test_psci_version() -> Result<(), ()> {
    expect_eq!(psci::version::<Smc>(), Err(psci::Error::NotSupported));
    Ok(())
}
