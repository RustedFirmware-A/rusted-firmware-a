// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Build script for RF-A.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(bti)");

    // This is necessary to ensure that cargo re-runs the build if one of the assembly files
    // included inside a macro with `#[include_first]` is changed.
    // TODO: Remove once `#[include_first]` handles this automatically.
    println!("cargo:rerun-if-changed=src");
}
