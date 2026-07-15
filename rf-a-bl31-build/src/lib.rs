// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script helpers for RF-A BL31.

use std::{env, error::Error, fmt::Display, path::Path};

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
    define_linker_symbol("CORE_COUNT", builder.core_count());
    define_linker_symbol("CACHE_WRITEBACK_GRANULE", builder.cache_writeback_granule());

    let linker_script_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("bl31.ld");
    add_linker_script(&linker_script_path);
}

/// Prints a line to stdout to make cargo use the given linker script.
fn add_linker_script(path: &Path) {
    println!("cargo:rustc-link-arg=-T{}", path.display());
    println!("cargo:rerun-if-changed={}", path.display());
}

/// Defines a global symbol in the linker script file, containing the absolute address given by
/// expression. For more information see the linker's description of `--defsym` parameter.
pub fn define_linker_symbol<D: Display>(symbol: &str, expression: D) {
    println!("cargo:rustc-link-arg=--defsym=\"{symbol}\"={expression}");
}

/// Result type for platform build configurations.
pub type BuildResult = Result<(), Box<dyn Error>>;

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

    /// Returns the core count of the target system.
    ///
    /// This is passed to the linker script through the `CORE_COUNT` symbol and made available to
    /// Rust code through the `CORE_COUNT` environment variable.
    fn core_count(&self) -> usize;

    /// Returns the cache writeback granule size in bytes.
    ///
    /// This is passed to the linker script through the `CACHE_WRITEBACK_GRANULE` symbol and made
    /// available to Rust code through the `CACHE_WRITEBACK_GRANULE` environment variable.
    fn cache_writeback_granule(&self) -> usize;

    /// Sets up platform-specific configurations (code generation, file inclusions, etc.).
    fn configure_build(&self) -> BuildResult {
        Ok(())
    }
}
