// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{BuildResult, Builder};
use cc::Build;

pub struct QemuBuilder;

impl QemuBuilder {
    pub const PLAT_NAME: &str = "qemu";

    const BL31_BASE: u64 = 0x0e09_0000;
    const BL31_SIZE: u64 = 0xa0000;
}

impl Builder for QemuBuilder {
    fn bl31_base(&self) -> u64 {
        Self::BL31_BASE
    }

    fn bl31_size(&self) -> u64 {
        Self::BL31_SIZE
    }

    fn configure_build(&self, build: &mut Build) -> BuildResult {
        if cfg!(feature = "rme") {
            Err(format!("RME is not supported on {:?}", QemuBuilder::PLAT_NAME).into())
        } else {
            build.include("platforms/qemu/include");
            Ok(())
        }
    }
}
