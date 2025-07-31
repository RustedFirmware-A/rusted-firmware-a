// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

mod config;

use super::{BuildResult, Builder};
use cc::Build;
use config::{FVP_CLUSTER_COUNT, FVP_MAX_CPUS_PER_CLUSTER, FVP_MAX_PE_PER_CPU};
use std::path::Path;

pub struct FvpBuilder;

impl FvpBuilder {
    pub const PLAT_NAME: &str = "fvp";

    const BL2_BASE: u64 = 0x0406_0000;
    const BL31_BASE: u64 = 0x0400_3000;
    const BL31_SIZE: u64 = Self::BL2_BASE - Self::BL31_BASE;
}

impl Builder for FvpBuilder {
    fn bl31_base(&self) -> u64 {
        Self::BL31_BASE
    }

    fn bl31_size(&self) -> u64 {
        Self::BL31_SIZE
    }

    fn configure_build(&self, build: &mut Build) -> BuildResult {
        build.include("platforms/fvp/include/");

        // TODO: Remove when .S files are re-written in Rust and this is no longer needed.
        build.define("FVP_CLUSTER_COUNT", FVP_CLUSTER_COUNT.to_string().as_str());
        build.define(
            "FVP_MAX_CPUS_PER_CLUSTER",
            FVP_MAX_CPUS_PER_CLUSTER.to_string().as_str(),
        );
        build.define(
            "FVP_MAX_PE_PER_CPU",
            FVP_MAX_PE_PER_CPU.to_string().as_str(),
        );

        let config_path = Path::new("platforms").join("fvp").join("config.rs");
        println!("cargo:rerun-if-changed={}", config_path.display());
        Ok(())
    }
}
