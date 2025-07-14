// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Test cases to run in secure world.

use crate::{
    expect_eq, secure_world_test,
    util::{
        current_el,
        timer::{SEL1Timer, SEL2Timer, test_timer_helper},
    },
};
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

secure_world_test!(test_secure_timer);
fn test_secure_timer() -> Result<(), ()> {
    if current_el() == 2 {
        // TODO: Enable SEL2Timer test for FVP.
        //
        // Right now ACKing a SEL2 interrupt in FVP
        // always returns special value 1023 (means spurious interrupt).
        // It looks like a bug in FVP itself.
        // Enable the test after figuring out what was the issue.
        #[cfg(platform = "fvp")]
        {
            log::warn!("SEL2 timer test skipped!");
            return Ok(());
        }

        test_timer_helper::<SEL2Timer>()
    } else {
        test_timer_helper::<SEL1Timer>()
    }
}
