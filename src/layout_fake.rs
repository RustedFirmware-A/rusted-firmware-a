// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub fn bl31_start() -> usize {
    0x6_0000
}

pub fn bl31_end() -> usize {
    0x10_0000
}

pub fn bl_code_base() -> usize {
    0x1_0000
}

pub fn bl_code_end() -> usize {
    0x3_0000
}

pub fn bl_ro_data_base() -> usize {
    0x3_0000
}

pub fn bl_ro_data_end() -> usize {
    0x4_0000
}

pub fn bss2_start() -> usize {
    0
}

pub fn bss2_end() -> usize {
    0
}
