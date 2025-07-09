// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A secure test framework.

use std::env;

/// The list of all supported platforms.
pub const PLATFORMS: &[&str] = &["fvp", "qemu"];

fn main() {
    let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \""),
    );
    println!("cargo:rustc-link-arg=-Timage.ld");
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    println!(
        "cargo:rustc-link-arg-bin=bl32=-T{}/{}_bl32.ld",
        crate_dir, platform
    );
    println!(
        "cargo:rustc-link-arg-bin=bl33=-T{}/{}_bl33.ld",
        crate_dir, platform
    );
    println!("cargo:rerun-if-changed={}/{}_bl32.ld", crate_dir, platform);
    println!("cargo:rerun-if-changed={}/{}_bl33.ld", crate_dir, platform);
}
