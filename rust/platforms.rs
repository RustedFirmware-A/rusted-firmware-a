// Copyright (c) 2024, Arm Limited. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

mod fvp;
mod qemu;

use cc::Build;
use fvp::FvpBuilder;
use qemu::QemuBuilder;
use std::error::Error;

pub const PLATFORMS: [&str; 2] = [QemuBuilder::PLAT_NAME, FvpBuilder::PLAT_NAME];

type BuildResult = Result<(), Box<dyn Error>>;

pub trait Builder {
    /// Sets up platform-specific configurations (code generation, file inclusions, `cc::Build`
    /// definitions, etc.).
    fn configure_build(&self, build: &mut Build) -> BuildResult;
}

pub fn get_builder(platform: &str) -> Result<Box<dyn Builder>, Box<dyn Error>> {
    match platform {
        FvpBuilder::PLAT_NAME => Ok(Box::new(FvpBuilder)),
        QemuBuilder::PLAT_NAME => Ok(Box::new(QemuBuilder)),
        _ => Err(format!(
            "Unexpected platform name {:?}. Supported platforms: {:?}",
            platform, PLATFORMS
        )
        .into()),
    }
}
