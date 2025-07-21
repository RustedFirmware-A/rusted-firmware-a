// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Helper functions to get addresses defined by the linker script.

unsafe extern "C" {
    // These aren't really variables, just symbols defined by the linker script whose addresses we
    // need to get. They should never be read or written.
    static __BL31_START__: u32;
    static __BL31_END__: u32;
    static __RODATA_START__: u32;
    static __RODATA_END__: u32;
    static __TEXT_START__: u32;
    static __TEXT_END__: u32;
}

/// Returns the address of the `__BL31_START__` symbol defined by the linker script.
pub fn bl31_start() -> usize {
    (&raw const __BL31_START__) as usize
}

/// Returns the address of the `__BL31_END__` symbol defined by the linker script.
pub fn bl31_end() -> usize {
    (&raw const __BL31_END__) as usize
}

/// Returns the address of the `__TEXT_START__` symbol defined by the linker script.
pub fn bl_code_base() -> usize {
    (&raw const __TEXT_START__) as usize
}

/// Returns the address of the `__TEXT_END__` symbol defined by the linker script.
pub fn bl_code_end() -> usize {
    (&raw const __TEXT_END__) as usize
}

/// Returns the address of the `__RODATA_START__` symbol defined by the linker script.
pub fn bl_ro_data_base() -> usize {
    (&raw const __RODATA_START__) as usize
}

/// Returns the address of the `__RODATA_END__` symbol defined by the linker script.
pub fn bl_ro_data_end() -> usize {
    (&raw const __RODATA_END__) as usize
}
