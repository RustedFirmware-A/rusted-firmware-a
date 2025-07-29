// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

mod platforms;

use platforms::{Builder, PLATFORMS, get_builder};
use std::env;

fn setup_linker(builder: &dyn Builder) {
    println!(
        "cargo:rustc-link-arg=--defsym=BL31_BASE={}",
        builder.bl31_base()
    );
    println!(
        "cargo:rustc-link-arg=--defsym=BL31_SIZE={}",
        builder.bl31_size()
    );

    println!("cargo:rustc-link-arg=-Tbl31.ld");
    println!("cargo:rerun-if-changed=bl31.ld");
}

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \""),
    );

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "none" {
        let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");

        let platform_builder = get_builder(&platform).unwrap();

        setup_linker(&*platform_builder);

        platform_builder.configure_build().unwrap();
    }
}
