// Copyright (c) 2023, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

#![no_main]
#![no_std]

use core::panic::PanicInfo;

#[no_mangle]
extern "C" fn bl31_main() {
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    loop {}
}
