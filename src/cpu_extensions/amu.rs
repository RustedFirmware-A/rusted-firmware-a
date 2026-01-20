// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Activity Monitor Unit (AMU) extension support.

use crate::{
    context::PerCoreState,
    cpu_extensions::CpuExtension,
    platform::{Platform, exception_free},
};
use arm_sysregs::{
    AmcrEl0, Amevcntr00El0, Amevcntr01El0, Amevcntr02El0, Amevcntr03El0, Amevcntr10El0,
    Amevcntr11El0, Amevcntr12El0, Amevcntr13El0, Amevcntr14El0, Amevcntr15El0, Amevcntr16El0,
    Amevcntr17El0, Amevcntr18El0, Amevcntr19El0, Amevcntr110El0, Amevcntr111El0, Amevcntr112El0,
    Amevcntr113El0, Amevcntr114El0, Amevcntr115El0, AmuserenrEl0, read_amcgcr_el0, read_amcr_el0,
    read_amevcntr00_el0, read_amevcntr01_el0, read_amevcntr02_el0, read_amevcntr03_el0,
    read_amevcntr10_el0, read_amevcntr11_el0, read_amevcntr12_el0, read_amevcntr13_el0,
    read_amevcntr14_el0, read_amevcntr15_el0, read_amevcntr16_el0, read_amevcntr17_el0,
    read_amevcntr18_el0, read_amevcntr19_el0, read_amevcntr110_el0, read_amevcntr111_el0,
    read_amevcntr112_el0, read_amevcntr113_el0, read_amevcntr114_el0, read_amevcntr115_el0,
    read_amuserenr_el0, read_id_aa64pfr0_el1, write_amcr_el0, write_amevcntr00_el0,
    write_amevcntr01_el0, write_amevcntr02_el0, write_amevcntr03_el0, write_amevcntr10_el0,
    write_amevcntr11_el0, write_amevcntr12_el0, write_amevcntr13_el0, write_amevcntr14_el0,
    write_amevcntr15_el0, write_amevcntr16_el0, write_amevcntr17_el0, write_amevcntr18_el0,
    write_amevcntr19_el0, write_amevcntr110_el0, write_amevcntr111_el0, write_amevcntr112_el0,
    write_amevcntr113_el0, write_amevcntr114_el0, write_amevcntr115_el0, write_amuserenr_el0,
};
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

#[derive(Clone, Copy, Default)]
struct AmuContext {
    amcr_el0: AmcrEl0,
    amuserenr_el0: AmuserenrEl0,
    amevcntr00_el0: Amevcntr00El0,
    amevcntr01_el0: Amevcntr01El0,
    amevcntr02_el0: Amevcntr02El0,
    amevcntr03_el0: Amevcntr03El0,
    amevcntr10_el0: Amevcntr10El0,
    amevcntr11_el0: Amevcntr11El0,
    amevcntr12_el0: Amevcntr12El0,
    amevcntr13_el0: Amevcntr13El0,
    amevcntr14_el0: Amevcntr14El0,
    amevcntr15_el0: Amevcntr15El0,
    amevcntr16_el0: Amevcntr16El0,
    amevcntr17_el0: Amevcntr17El0,
    amevcntr18_el0: Amevcntr18El0,
    amevcntr19_el0: Amevcntr19El0,
    amevcntr110_el0: Amevcntr110El0,
    amevcntr111_el0: Amevcntr111El0,
    amevcntr112_el0: Amevcntr112El0,
    amevcntr113_el0: Amevcntr113El0,
    amevcntr114_el0: Amevcntr114El0,
    amevcntr115_el0: Amevcntr115El0,
}

impl AmuContext {
    const EMPTY: Self = Self {
        amcr_el0: AmcrEl0::empty(),
        amuserenr_el0: AmuserenrEl0::empty(),
        amevcntr00_el0: Amevcntr00El0::empty(),
        amevcntr01_el0: Amevcntr01El0::empty(),
        amevcntr02_el0: Amevcntr02El0::empty(),
        amevcntr03_el0: Amevcntr03El0::empty(),
        amevcntr10_el0: Amevcntr10El0::empty(),
        amevcntr11_el0: Amevcntr11El0::empty(),
        amevcntr12_el0: Amevcntr12El0::empty(),
        amevcntr13_el0: Amevcntr13El0::empty(),
        amevcntr14_el0: Amevcntr14El0::empty(),
        amevcntr15_el0: Amevcntr15El0::empty(),
        amevcntr16_el0: Amevcntr16El0::empty(),
        amevcntr17_el0: Amevcntr17El0::empty(),
        amevcntr18_el0: Amevcntr18El0::empty(),
        amevcntr19_el0: Amevcntr19El0::empty(),
        amevcntr110_el0: Amevcntr110El0::empty(),
        amevcntr111_el0: Amevcntr111El0::empty(),
        amevcntr112_el0: Amevcntr112El0::empty(),
        amevcntr113_el0: Amevcntr113El0::empty(),
        amevcntr114_el0: Amevcntr114El0::empty(),
        amevcntr115_el0: Amevcntr115El0::empty(),
    };

    fn save(&mut self) {
        self.amcr_el0 = read_amcr_el0();
        self.amuserenr_el0 = read_amuserenr_el0();

        let n_group1 = read_amcgcr_el0().cg1nc();

        self.amevcntr00_el0 = read_amevcntr00_el0();
        self.amevcntr01_el0 = read_amevcntr01_el0();
        self.amevcntr02_el0 = read_amevcntr02_el0();
        self.amevcntr03_el0 = read_amevcntr03_el0();

        if n_group1 > 0 {
            self.amevcntr10_el0 = read_amevcntr10_el0();
        }
        if n_group1 > 1 {
            self.amevcntr11_el0 = read_amevcntr11_el0();
        }
        if n_group1 > 2 {
            self.amevcntr12_el0 = read_amevcntr12_el0();
        }
        if n_group1 > 3 {
            self.amevcntr13_el0 = read_amevcntr13_el0();
        }
        if n_group1 > 4 {
            self.amevcntr14_el0 = read_amevcntr14_el0();
        }
        if n_group1 > 5 {
            self.amevcntr15_el0 = read_amevcntr15_el0();
        }
        if n_group1 > 6 {
            self.amevcntr16_el0 = read_amevcntr16_el0();
        }
        if n_group1 > 7 {
            self.amevcntr17_el0 = read_amevcntr17_el0();
        }
        if n_group1 > 8 {
            self.amevcntr18_el0 = read_amevcntr18_el0();
        }
        if n_group1 > 9 {
            self.amevcntr19_el0 = read_amevcntr19_el0();
        }
        if n_group1 > 10 {
            self.amevcntr110_el0 = read_amevcntr110_el0();
        }
        if n_group1 > 11 {
            self.amevcntr111_el0 = read_amevcntr111_el0();
        }
        if n_group1 > 12 {
            self.amevcntr112_el0 = read_amevcntr112_el0();
        }
        if n_group1 > 13 {
            self.amevcntr113_el0 = read_amevcntr113_el0();
        }
        if n_group1 > 14 {
            self.amevcntr114_el0 = read_amevcntr114_el0();
        }
        if n_group1 > 15 {
            self.amevcntr115_el0 = read_amevcntr115_el0();
        }
    }

    fn restore(&self) {
        // SAFETY: We're restoring the values previously saved, so they must be valid.
        unsafe {
            // Disable all counters before restoring.
            write_amcr_el0(AmcrEl0::empty());

            let n_group1 = read_amcgcr_el0().cg1nc();

            write_amevcntr00_el0(self.amevcntr00_el0);
            write_amevcntr01_el0(self.amevcntr01_el0);
            write_amevcntr02_el0(self.amevcntr02_el0);
            write_amevcntr03_el0(self.amevcntr03_el0);

            if n_group1 > 0 {
                write_amevcntr10_el0(self.amevcntr10_el0);
            }
            if n_group1 > 1 {
                write_amevcntr11_el0(self.amevcntr11_el0);
            }
            if n_group1 > 2 {
                write_amevcntr12_el0(self.amevcntr12_el0);
            }
            if n_group1 > 3 {
                write_amevcntr13_el0(self.amevcntr13_el0);
            }
            if n_group1 > 4 {
                write_amevcntr14_el0(self.amevcntr14_el0);
            }
            if n_group1 > 5 {
                write_amevcntr15_el0(self.amevcntr15_el0);
            }
            if n_group1 > 6 {
                write_amevcntr16_el0(self.amevcntr16_el0);
            }
            if n_group1 > 7 {
                write_amevcntr17_el0(self.amevcntr17_el0);
            }
            if n_group1 > 8 {
                write_amevcntr18_el0(self.amevcntr18_el0);
            }
            if n_group1 > 9 {
                write_amevcntr19_el0(self.amevcntr19_el0);
            }
            if n_group1 > 10 {
                write_amevcntr110_el0(self.amevcntr110_el0);
            }
            if n_group1 > 11 {
                write_amevcntr111_el0(self.amevcntr111_el0);
            }
            if n_group1 > 12 {
                write_amevcntr112_el0(self.amevcntr112_el0);
            }
            if n_group1 > 13 {
                write_amevcntr113_el0(self.amevcntr113_el0);
            }
            if n_group1 > 14 {
                write_amevcntr114_el0(self.amevcntr114_el0);
            }
            if n_group1 > 15 {
                write_amevcntr115_el0(self.amevcntr115_el0);
            }

            write_amuserenr_el0(self.amuserenr_el0);
            write_amcr_el0(self.amcr_el0);
        }
    }
}

/// Activity Monitor Unit (AMU) extension support.
pub struct Amu<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, AmuContext>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Amu<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the AMU CPU extension.
    #[allow(dead_code)]
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const { ExceptionLock::new(RefCell::new(AmuContext::EMPTY)) }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Amu<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Amu<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64pfr0_el1().is_feat_amu_present()
    }

    fn save_context_before_suspend_to_powerdown(&self) {
        if !self.is_present() {
            return;
        }

        exception_free(|token| {
            let mut ctx = self.context.get().borrow_mut(token);
            ctx.save();
        });
    }

    fn restore_context_after_suspend_to_powerdown(&self) {
        if !self.is_present() {
            return;
        }

        exception_free(|token| {
            let ctx = self.context.get().borrow_mut(token);
            ctx.restore();
        });
    }
}
