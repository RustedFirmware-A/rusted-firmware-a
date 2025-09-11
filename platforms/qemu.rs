// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Builder;

pub struct QemuBuilder;

impl QemuBuilder {
    pub const PLAT_NAME: &str = "qemu";

    const BL31_BASE: u64 = 0x0e09_0000;
    const BL31_SIZE: u64 = Self::BL31_DRAM_BASE - Self::BL31_BASE;

    // This isn't really DRAM, we just use some secure memory before BL32 as BSS2 for the sake of
    // exercising the feature.
    const BL31_DRAM_BASE: u64 = Self::BL32_BASE - Self::BL31_DRAM_SIZE;
    const BL31_DRAM_SIZE: u64 = 0x0000_1000;

    const BL32_BASE: u64 = 0x0e10_0000;
}

impl Builder for QemuBuilder {
    fn bl31_base(&self) -> u64 {
        Self::BL31_BASE
    }

    fn bl31_size(&self) -> u64 {
        Self::BL31_SIZE
    }

    fn bl31_dram_base(&self) -> Option<u64> {
        Some(Self::BL31_DRAM_BASE)
    }

    fn bl31_dram_size(&self) -> u64 {
        Self::BL31_DRAM_SIZE
    }
}
