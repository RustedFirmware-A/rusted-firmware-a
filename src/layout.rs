// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Helper functions to get addresses defined by the linker script.

#[allow(improper_ctypes)]
unsafe extern "C" {
    // These aren't really variables, just symbols defined by the linker script whose addresses we
    // need to get. They should never be read or written. We must use zero-sized types for them
    // because some of them may have the same address, and Rust requires statics not to overlap.
    static __BL31_START__: ();
    static __BL31_END__: ();
    static __RODATA_START__: ();
    static __RODATA_END__: ();
    static __TEXT_START__: ();
    static __TEXT_END__: ();
    static __BSS2_START__: ();
    static __BSS2_END__: ();
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

/// Returns the address of the `__BL31_SEC_DRAM_START__` symbol defined by the linker script.
pub fn bss2_start() -> usize {
    (&raw const __BSS2_START__) as usize
}

/// Returns the address of the `__BL31_SEC_DRAM_END__` symbol defined by the linker script.
pub fn bss2_end() -> usize {
    (&raw const __BSS2_END__) as usize
}
