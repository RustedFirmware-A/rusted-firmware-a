// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

mod platforms;

use platforms::{Builder, PLATFORMS, add_linker_script, define_linker_symbol, get_builder};
use std::{env, path::PathBuf};

/// One page of memory has 4KiB.
const PAGE_SIZE: u64 = 0x1000;

fn setup_linker(builder: &dyn Builder) {
    define_linker_symbol("BL31_BASE", builder.bl31_base());
    define_linker_symbol("BL31_SIZE", builder.bl31_size());
    define_linker_symbol("PAGE_SIZE", PAGE_SIZE);

    add_linker_script(&PathBuf::from("bl31.ld"));
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
