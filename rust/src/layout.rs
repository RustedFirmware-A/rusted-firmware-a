// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Helper functions to get addresses defined by the linker script.

use core::ptr::addr_of;

extern "C" {
    // These aren't really variables, just symbols defined by the linker script whose addresses we
    // need to get. They should never be read or written.
    static __BL31_END__: u32;
    static __RODATA_START__: u32;
    static __RODATA_END__: u32;
    static __TEXT_START__: u32;
    static __TEXT_END__: u32;
}

/// Returns the address of the `__BL31_END__` symbol defined by the linker script.
pub fn bl31_end() -> usize {
    // SAFETY: We're just getting the address of a symbol defined by the linker script, not actually
    // accessing the memory behind it.
    unsafe { addr_of!(__BL31_END__) as usize }
}

/// Returns the address of the `__TEXT_START__` symbol defined by the linker script.
pub fn bl_code_base() -> usize {
    // SAFETY: We're just getting the address of a symbol defined by the linker script, not actually
    // accessing the memory behind it.
    unsafe { addr_of!(__TEXT_START__) as usize }
}

/// Returns the address of the `__TEXT_END__` symbol defined by the linker script.
pub fn bl_code_end() -> usize {
    // SAFETY: We're just getting the address of a symbol defined by the linker script, not actually
    // accessing the memory behind it.
    unsafe { addr_of!(__TEXT_END__) as usize }
}

/// Returns the address of the `__RODATA_START__` symbol defined by the linker script.
pub fn bl_ro_data_base() -> usize {
    // SAFETY: We're just getting the address of a symbol defined by the linker script, not actually
    // accessing the memory behind it.
    unsafe { addr_of!(__RODATA_START__) as usize }
}

/// Returns the address of the `__RODATA_END__` symbol defined by the linker script.
pub fn bl_ro_data_end() -> usize {
    // SAFETY: We're just getting the address of a symbol defined by the linker script, not actually
    // accessing the memory behind it.
    unsafe { addr_of!(__RODATA_END__) as usize }
}
