// Copyright (c) 2024, Arm Limited. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Builder;
use anyhow::bail;
use cc::Build;

pub struct QemuBuilder;

impl Builder for QemuBuilder {
    fn configure_build(&self, build: &mut Build) -> anyhow::Result<()> {
        if cfg!(feature = "rme") {
            bail!("RME is not supported on {:?}", QemuBuilder::PLAT_NAME);
        }

        build
            .include("../plat/qemu/common/include")
            .include("../plat/qemu/qemu/include")
            .file("../plat/qemu/common/aarch64/plat_helpers.S");
        Ok(())
    }
}

impl QemuBuilder {
    pub const PLAT_NAME: &str = "qemu";
}
