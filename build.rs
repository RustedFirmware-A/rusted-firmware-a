// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

mod platforms;

use cc::Build;
use platforms::{Builder, PLATFORMS, get_builder};
use std::env;

fn build_libtfa(platform_builder: &dyn Builder) {
    // SAFETY: The build script is single-threaded.
    unsafe {
        env::set_var("CROSS_COMPILE", "aarch64-none-elf");
        env::set_var("CC", "clang");
    }

    let mut build = Build::new();
    if env::var("CARGO_FEATURE_RME").as_deref() == Ok("1") {
        build.define("ENABLE_RME", Some("1"));
    }
    build
        .define("CRASH_REPORTING", Some("1"))
        .define("ENABLE_ASSERTIONS", Some("1"))
        .include("include")
        .include("include/arch/aarch64")
        .include("include/lib/cpus/aarch64")
        .include("include/lib/el3_runtime/aarch64")
        .include("include/lib/libc")
        .include("include/plat/arm/common/aarch64")
        .file("cpu_data.S");

    if let Ok(debug) = env::var("DEBUG") {
        build.define("DEBUG", debug.as_str());
    }

    platform_builder.configure_build(&mut build).unwrap();

    build.compile("tfa");
}

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

        build_libtfa(&*platform_builder);

        setup_linker(&*platform_builder);
    }
}
