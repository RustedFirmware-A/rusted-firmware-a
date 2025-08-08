// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

mod fvp;
mod qemu;

use fvp::FvpBuilder;
use qemu::QemuBuilder;
use std::{error::Error, path::Path};

pub const PLATFORMS: [&str; 2] = [QemuBuilder::PLAT_NAME, FvpBuilder::PLAT_NAME];

type BuildResult = Result<(), Box<dyn Error>>;

pub trait Builder {
    /// Base address of the BL31 binary.
    ///
    /// Passed to platform-independent linker script by
    /// defining BL31_BASE symbol.
    fn bl31_base(&self) -> u64;

    /// Size of the BL31 binary.
    ///
    /// Passed to platform-independent linker script by
    /// defining BL31_SIZE symbol.
    fn bl31_size(&self) -> u64;

    /// Sets up platform-specific configurations (code generation, file inclusions, etc.).
    fn configure_build(&self) -> BuildResult {
        Ok(())
    }
}

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

pub fn add_linker_script(path: &Path) {
    println!("cargo:rustc-link-arg=-T{}", path.display());
    println!("cargo:rerun-if-changed={}", path.display());
}

pub fn define_linker_symbol(name: &str, value: u64) {
    println!("cargo:rustc-link-arg=--defsym=\"{name}\"={value}");
}
