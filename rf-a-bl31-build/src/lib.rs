// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script helpers for RF-A BL31.

use std::{env, error::Error, fs::File, io::Write, path::Path};

/// One page of memory has 4KiB.
const PAGE_SIZE: u64 = 0x1000;

/// Configures the build for the given platform.
pub fn configure_build(builder: &dyn Builder) {
    setup_linker(builder);
    builder.configure_build().unwrap();
}

/// Sets up the linker configuration for the given platform builder.
fn setup_linker(builder: &dyn Builder) {
    if builder.bl31_dram_base().is_none() {
        assert_eq!(builder.bl31_dram_size(), 0);
    }

    define_linker_symbol("BL31_BASE", builder.bl31_base());
    define_linker_symbol("BL31_SIZE", builder.bl31_size());
    define_linker_symbol(
        "BL31_DRAM_BASE",
        builder.bl31_dram_base().unwrap_or_default(),
    );
    define_linker_symbol("BL31_DRAM_SIZE", builder.bl31_dram_size());
    define_linker_symbol("PAGE_SIZE", PAGE_SIZE);

    // Write linker script to the out directory, so that the binary build can find it.
    let linker_script_path = Path::new(&env::var_os("OUT_DIR").unwrap()).join("bl31.ld");
    File::create(&linker_script_path)
        .unwrap()
        .write_all(include_bytes!("bl31.ld"))
        .unwrap();
    add_linker_script(&linker_script_path);
}

/// Prints a line to stdout to make cargo use the given linker script.
fn add_linker_script(path: &Path) {
    println!("cargo:rustc-link-arg=-T{}", path.display());
    println!("cargo:rerun-if-changed={}", path.display());
}

/// Prints a line to stdout to make cargo define the given symbol for linker scripts.
fn define_linker_symbol(name: &str, value: u64) {
    println!("cargo:rustc-link-arg=--defsym=\"{name}\"={value}");
}

type BuildResult = Result<(), Box<dyn Error>>;

/// Trait implemented by each platform.
pub trait Builder {
    /// Base address of the BL31 binary.
    ///
    /// This is passed to the linker script through the `BL31_BASE` symbol.
    fn bl31_base(&self) -> u64;

    /// Size of the BL31 binary.
    ///
    /// This is passed to the linker script through the `BL31_SIZE` symbol.
    fn bl31_size(&self) -> u64;

    /// Base address of the DRAM section reserved for BL31, if any.
    ///
    /// If no DRAM is reserved for BL31 then this should return `None`.
    ///
    /// This is passed to the linker script through the `BL31_DRAM_BASE` symbol.
    fn bl31_dram_base(&self) -> Option<u64> {
        None
    }

    /// Size of the DRAM section reserved for BL31, if any.
    ///
    /// If no DRAM is reserved for BL31 then this should return 0.
    ///
    /// This is passed to the linker script through the `BL31_DRAM_SIZE` symbol.
    fn bl31_dram_size(&self) -> u64 {
        0
    }

    /// Sets up platform-specific configurations (code generation, file inclusions, etc.).
    fn configure_build(&self) -> BuildResult {
        Ok(())
    }
}
