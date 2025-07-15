// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

mod platforms;

use cc::Build;
use platforms::{get_builder, PLATFORMS};
use std::env;

fn build_libtfa(platform: &str) {
    let platform_builder = get_builder(platform).unwrap();

    env::set_var("CROSS_COMPILE", "aarch64-none-elf");
    env::set_var("CC", "clang");

    let mut build = Build::new();
    if env::var("CARGO_FEATURE_RME").as_deref() == Ok("1") {
        build.define("ENABLE_RME", Some("1"));
    }
    build
        .define("CRASH_REPORTING", Some("1"))
        .define("PL011_GENERIC_UART", Some("0"))
        .define("ENABLE_ASSERTIONS", Some("1"))
        .define("ENABLE_CONSOLE_GETC", Some("0"))
        .define("IMAGE_BL31", Some("1"))
        .include("include")
        .include("include/arch/aarch64")
        .include("include/lib/cpus/aarch64")
        .include("include/lib/el3_runtime/aarch64")
        .include("include/lib/libc")
        .include("include/plat/arm/common/aarch64")
        .file("bl31_entrypoint.S")
        .file("pl011_console.S")
        .file("cpu_helpers.S")
        .file("cpu_data.S")
        .file("enable_mmu.S");

    if let Ok(debug) = env::var("DEBUG") {
        build.define("DEBUG", debug.as_str());
    }

    platform_builder.configure_build(&mut build).unwrap();

    build.compile("tfa");
}

fn setup_linker(platform: &String) {
    println!("cargo:rustc-link-arg=-Tbl31.ld");
    println!("cargo:rerun-if-changed=bl31.ld");

    // Select the linker scripts. bl31.ld is common to all platforms. It gets supplemented by the
    // platform linker script. Some platforms have multiple linker scripts, depending on the enabled
    // features.
    let linker_name = platform.clone();
    #[cfg(feature = "rme")]
    let linker_name = linker_name + "-rme";

    let linker_name = format!("platforms/{}/{}.ld", platform, linker_name);
    println!("cargo:rustc-link-arg=-T{}", linker_name);
    println!("cargo:rerun-if-changed={}", linker_name);
}

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \""),
    );

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "none" {
        let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");

        build_libtfa(&platform);

        setup_linker(&platform);
    }
}
