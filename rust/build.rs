// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use cc::Build;
use std::env;

const PLATFORMS: [&str; 2] = ["qemu", "fvp"];

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \"")
    );

    let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");
    assert!(
        PLATFORMS.contains(&platform.as_str()),
        "Unexpected platform name {:?}. Supported platforms: {:?}",
        platform,
        PLATFORMS,
    );

    env::set_var("CROSS_COMPILE", "aarch64-none-elf");
    env::set_var("CC", "clang");

    let mut build = Build::new();
    build
        .define("CRASH_REPORTING", Some("1"))
        .define("PL011_GENERIC_UART", Some("0"))
        .define("ENABLE_ASSERTIONS", Some("1"))
        .define("ENABLE_CONSOLE_GETC", Some("0"))
        .define("IMAGE_BL31", Some("1"))
        .include("../include")
        .include("../include/arch/aarch64")
        .include("../include/lib/cpus/aarch64")
        .include("../include/lib/el3_runtime/aarch64")
        .include("../include/lib/libc")
        .include("../include/plat/arm/common/aarch64")
        .include("../plat/qemu/common/include")
        .include("../plat/qemu/qemu/include")
        .file("bl31_entrypoint.S")
        .file("context.S")
        .file("crash_reporting.S")
        .file("debug.S")
        .file("platform_helpers.S")
        .file("runtime_exceptions.S")
        .file("../drivers/arm/pl011/aarch64/pl011_console.S")
        .file("../plat/qemu/common/aarch64/plat_helpers.S")
        .file("../lib/aarch64/cache_helpers.S")
        .file("../lib/aarch64/misc_helpers.S")
        .file("../lib/cpus/aarch64/cpu_helpers.S")
        .file("../lib/el3_runtime/aarch64/cpu_data.S")
        .file("../lib/xlat_tables_v2/aarch64/enable_mmu.S");

    if let Ok(debug) = env::var("DEBUG") {
        build.define("DEBUG", debug.as_str());
    }

    build.compile("empty");

    println!("cargo:rustc-link-arg=-Timage.ld");
    println!("cargo:rustc-link-arg=-T{}.ld", platform);
}
