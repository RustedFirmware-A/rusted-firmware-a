// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

mod fvp;
mod qemu;

use fvp::FvpBuilder;
use qemu::QemuBuilder;
use rf_a_bl31_build::Builder;
use std::error::Error;

pub const PLATFORMS: [&str; 2] = [QemuBuilder::PLAT_NAME, FvpBuilder::PLAT_NAME];

pub fn get_builder(platform: &str) -> Result<Box<dyn Builder>, Box<dyn Error>> {
    match platform {
        FvpBuilder::PLAT_NAME => Ok(Box::new(FvpBuilder)),
        QemuBuilder::PLAT_NAME => Ok(Box::new(QemuBuilder)),
        _ => Err(format!(
            "Unexpected platform name {platform:?}. Supported platforms: {PLATFORMS:?}"
        )
        .into()),
    }
}
