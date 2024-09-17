// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    context::EntryPointInfo,
    gicv3,
    pagetable::{map_region, IdMap, MT_DEVICE},
    services::arch::WorkaroundSupport,
    sysregs::SpsrEl3,
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::gicv3::GicV3;
use percore::{Cores, ExceptionFree};

const DEVICE0_BASE: usize = 0x0200_0000;
const DEVICE0_SIZE: usize = 0x1000;
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);

/// A fake platform for unit tests.
pub struct TestPlatform;

impl Platform for TestPlatform {
    const CORE_COUNT: usize = 1;

    type LoggerWriter = DummyLoggerWriter;

    const GIC_CONFIG: gicv3::GicConfig = gicv3::GicConfig {
        secure_interrupts_config: &[],
    };

    fn init_beforemmu() {}

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &DEVICE0, MT_DEVICE);
    }

    unsafe fn create_gic() -> GicV3 {
        unimplemented!();
    }

    fn secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x4000_0000,
            spsr: SpsrEl3::M_AARCH64_EL1T,
            args: Default::default(),
        }
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x6000_0000,
            spsr: SpsrEl3::M_AARCH64_EL1T,
            args: Default::default(),
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x2000_0000,
            spsr: 0x3c9,
            args: Default::default(),
        }
    }

    fn system_off() -> ! {
        panic!("system_off called in test.");
    }

    fn arch_workaround_1_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_1() {}

    fn arch_workaround_2_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_2() {}

    fn arch_workaround_3_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_3() {}

    fn arch_workaround_4_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }
}

// SAFETY: The `TestPlatform` pretends to have 1 core, and `core_index` always returns 0. This
// trivially satisfies `Cores`' safety requirement that it not return the same index on different
// cores.
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

pub struct DummyLoggerWriter;

impl core::fmt::Write for DummyLoggerWriter {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        Ok(())
    }
}
