// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

mod platforms;

use platforms::{PLATFORMS, get_builder};
use rf_a_bl31_build::configure_build;
use std::env;

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \""),
    );
    println!("cargo::rustc-check-cfg=cfg(bti)");

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "none" {
        let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");

        let platform_builder = get_builder(&platform).unwrap();

        configure_build(&*platform_builder);
    }

    // This is necessary to ensure that cargo re-runs the build if one of the assembly files
    // included inside a macro with `#[include_first]` is changed.
    // TODO: Remove once `#[include_first]` handles this automatically.
    println!("cargo:rerun-if-changed=src");
}
