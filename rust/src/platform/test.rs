// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{context::EntryPointInfo, pagetable::IdMap};
use percore::{Cores, ExceptionFree};

pub const BL31_BASE: usize = 0x6_0000;

/// A fake platform for unit tests.
pub struct TestPlatform;

impl Platform for TestPlatform {
    const CORE_COUNT: usize = 1;

    fn init_beforemmu() {}

    fn map_extra_regions(_idmap: &mut IdMap) {}

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x60000000,
            spsr: 0x04,
            args: Default::default(),
        }
    }

    fn system_off() -> ! {
        panic!("system_off called in test.");
    }
}

unsafe impl Cores for TestPlatform {
    fn core_index() -> usize {
        0
    }
}

/// Runs the given function and returns the result.
///
/// This is a fake version of `percore::exception_free` for use in unit tests only, which must be
/// run on a single thread.
pub fn exception_free<T>(f: impl FnOnce(ExceptionFree) -> T) -> T {
    // SAFETY: This is only used in unit tests, which are run on the host where there are no
    // hardware exceptions nor multiple threads.
    let token = unsafe { ExceptionFree::new() };
    f(token)
}
