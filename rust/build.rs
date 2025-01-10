// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

mod platforms;
use platforms::{get_builder, PLATFORMS};

use cc::Build;
use std::env;

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \""),
    );

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "none" {
        let platform = env::var("CARGO_CFG_PLATFORM").expect("Missing platform name");

        let platform_builder = get_builder(platform.as_str()).unwrap();

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
            .include("../include")
            .include("../include/arch/aarch64")
            .include("../include/lib/cpus/aarch64")
            .include("../include/lib/el3_runtime/aarch64")
            .include("../include/lib/libc")
            .include("../include/plat/arm/common/aarch64")
            .file("bl31_entrypoint.S")
            .file("context.S")
            .file("crash_reporting.S")
            .file("debug.S")
            .file("platform_helpers.S")
            .file("runtime_exceptions.S")
            .file("../drivers/arm/pl011/aarch64/pl011_console.S")
            .file("../lib/aarch64/cache_helpers.S")
            .file("../lib/aarch64/misc_helpers.S")
            .file("../lib/cpus/aarch64/cpu_helpers.S")
            .file("../lib/el3_runtime/aarch64/cpu_data.S")
            .file("../lib/xlat_tables_v2/aarch64/enable_mmu.S");

        if let Ok(debug) = env::var("DEBUG") {
            build.define("DEBUG", debug.as_str());
        }

        platform_builder.configure_build(&mut build).unwrap();

        build.compile("tfa");

        println!("cargo:rustc-link-arg=-Timage.ld");

        // Select the linker scripts.
        // image.ld is common to all platforms.
        // It gets supplemented by the platform linker script.
        // Some platforms have multiple of those, depending on the enabled features.
        #[allow(unused)]
        let mut linker_name = platform.clone();

        #[cfg(feature = "rme")]
        linker_name.push_str("-rme");

        println!(
            "cargo:rustc-link-arg=-Tplatforms/{}/{}.ld",
            platform, linker_name
        );
    }
}
