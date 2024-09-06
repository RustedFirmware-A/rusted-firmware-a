// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use cc::Build;
use std::env;
use std::fs;
use std::path::Path;

const PLATFORMS: [&str; 2] = ["qemu", "fvp"];

fn main() {
    println!(
        "cargo::rustc-check-cfg=cfg(platform, values(\"{}\"))",
        PLATFORMS.join("\", \"")
    );

    if env::var("CARGO_CFG_TARGET_OS").unwrap() == "none" {
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

        if platform.as_str().eq("fvp") {
            build
                .file("../plat/arm/common/aarch64/arm_helpers.S")
                .include("../include/plat/arm/common")
                .include("../plat/arm/board/fvp/include");

            // Get compile time config values from ENV vars
            //   1. to pass as ENV vars to Build::
            //   2. to generate `pub const` code in fvp_defines.rs for use in firmware Rust code
            let code_gen: String = [
                // env var, type, default value
                ("FVP_CLUSTER_COUNT", "usize", "2"),
                ("FVP_MAX_CPUS_PER_CLUSTER", "usize", "4"),
                ("FVP_MAX_PE_PER_CPU", "usize", "1"),
            ]
            .into_iter()
            .map(|(env_var, var_type, default)| {
                let val = env::var(env_var).unwrap_or(default.to_string());
                build.define(env_var, val.as_str());
                format!("pub const {} : {} = {};\n", env_var, var_type, val)
            })
            .collect::<Vec<String>>()
            .join("");

            let out_dir = env::var_os("OUT_DIR").unwrap();
            let dest_path = Path::new(&out_dir).join("fvp_defines.rs");
            fs::write(&dest_path, code_gen).unwrap();
        } else if platform.as_str().eq("qemu") {
            build
                .include("../plat/qemu/common/include")
                .include("../plat/qemu/qemu/include")
                .file("../plat/qemu/common/aarch64/plat_helpers.S");
        }
        build.compile("empty");

        println!("cargo:rustc-link-arg=-Timage.ld");
        println!("cargo:rustc-link-arg=-T{}.ld", platform);
    }
}
