// Copyright (c) 2024, Arm Limited. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

mod fvp;
mod qemu;

use anyhow::anyhow;
use cc::Build;
use fvp::FvpBuilder;
use qemu::QemuBuilder;

pub const PLATFORMS: [&str; 2] = [QemuBuilder::PLAT_NAME, FvpBuilder::PLAT_NAME];

pub trait Builder {
    // Add platform specific configurations (code generation, file inclusions cc::Build definitions,etc.)
    fn configure_build(&self, build: &mut Build) -> anyhow::Result<()>;
}

pub fn get_builder(platform: &str) -> anyhow::Result<Box<dyn Builder>> {
    match platform {
        FvpBuilder::PLAT_NAME => Ok(Box::new(FvpBuilder)),
        QemuBuilder::PLAT_NAME => Ok(Box::new(QemuBuilder)),
        _ => Err(anyhow!(
            "Unexpected platform name {:?}. Supported platforms: {:?}",
            platform,
            PLATFORMS
        )),
    }
}
