// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{BuildResult, Builder};
use cc::Build;

pub struct QemuBuilder;

impl Builder for QemuBuilder {
    fn configure_build(&self, build: &mut Build) -> BuildResult {
        if cfg!(feature = "rme") {
            Err(format!("RME is not supported on {:?}", QemuBuilder::PLAT_NAME).into())
        } else {
            build
                .include("platforms/qemu/include")
                .file("platforms/qemu/plat_helpers.S");
            Ok(())
        }
    }
}

impl QemuBuilder {
    pub const PLAT_NAME: &str = "qemu";
}
