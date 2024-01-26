// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use cc::Build;
use std::env;

fn main() {
    env::set_var("CROSS_COMPILE", "aarch64-none-elf");
    env::set_var("CC", "clang");

    Build::new()
        .include("../include")
        .include("../include/arch/aarch64")
        .include("../include/lib/el3_runtime/aarch64")
        .include("../plat/qemu/qemu/include")
        .file("bl31_entrypoint.S")
        .file("../lib/aarch64/misc_helpers.S")
        .compile("empty")
}
