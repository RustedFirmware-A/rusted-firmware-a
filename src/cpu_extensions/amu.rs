// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Activity Monitor Unit (AMU) extension support.

use crate::{
    aarch64::isb,
    context::{CPU_DATA_CONTEXT_NUM, PerWorld, PerWorldContext, World},
    cpu_extensions::CpuExtension,
    platform::exception_free,
};
use arm_sysregs::{
    el0::{
        accessors::{
            read_amcg1idr_el0, read_amcgcr_el0, read_amcr_el0, read_amevcntr00_el0,
            read_amevcntr01_el0, read_amevcntr02_el0, read_amevcntr03_el0, read_amevcntr10_el0,
            read_amevcntr11_el0, read_amevcntr12_el0, read_amevcntr13_el0, read_amevcntr14_el0,
            read_amevcntr15_el0, read_amevcntr16_el0, read_amevcntr17_el0, read_amevcntr18_el0,
            read_amevcntr19_el0, read_amevcntr110_el0, read_amevcntr111_el0, read_amevcntr112_el0,
            read_amevcntr113_el0, read_amevcntr114_el0, read_amevcntr115_el0, read_amuserenr_el0,
            write_amcntenclr0_el0, write_amcntenclr1_el0, write_amcntenset0_el0,
            write_amcntenset1_el0, write_amcr_el0, write_amevcntr00_el0, write_amevcntr01_el0,
            write_amevcntr02_el0, write_amevcntr03_el0, write_amevcntr10_el0, write_amevcntr11_el0,
            write_amevcntr12_el0, write_amevcntr13_el0, write_amevcntr14_el0, write_amevcntr15_el0,
            write_amevcntr16_el0, write_amevcntr17_el0, write_amevcntr18_el0, write_amevcntr19_el0,
            write_amevcntr110_el0, write_amevcntr111_el0, write_amevcntr112_el0,
            write_amevcntr113_el0, write_amevcntr114_el0, write_amevcntr115_el0,
            write_amuserenr_el0,
        },
        registers::{
            Amcg1idrEl0, Amcntenclr0El0, Amcntenclr1El0, Amcntenset0El0, Amcntenset1El0, AmcrEl0,
            Amevcntr00El0, Amevcntr01El0, Amevcntr02El0, Amevcntr03El0, Amevcntr10El0,
            Amevcntr11El0, Amevcntr12El0, Amevcntr13El0, Amevcntr14El0, Amevcntr15El0,
            Amevcntr16El0, Amevcntr17El0, Amevcntr18El0, Amevcntr19El0, Amevcntr110El0,
            Amevcntr111El0, Amevcntr112El0, Amevcntr113El0, Amevcntr114El0, Amevcntr115El0,
            AmuserenrEl0,
        },
    },
    el1::accessors::read_id_aa64pfr0_el1,
    el3::registers::{CptrEl3, ScrEl3},
};
use core::{cell::RefCell, marker::PhantomData};
use percore::{Cores, ExceptionLock, derive::percore};

const GROUP0_ALWAYS_ON: Amcntenclr0El0 = Amcntenclr0El0::P0.union(Amcntenclr0El0::P1);
const GROUP0_CONTEXTED: Amcntenclr0El0 = Amcntenclr0El0::P2.union(Amcntenclr0El0::P3);
const GROUP0_ALL: Amcntenclr0El0 = GROUP0_ALWAYS_ON.union(GROUP0_CONTEXTED);
const GROUP1_ALL: Amcntenclr1El0 = Amcntenclr1El0::all();

struct AmuWorldContext {
    amevcntr02_el0: Amevcntr02El0,
    amevcntr03_el0: Amevcntr03El0,
}

impl AmuWorldContext {
    const EMPTY: Self = Self {
        amevcntr02_el0: Amevcntr02El0::empty(),
        amevcntr03_el0: Amevcntr03El0::empty(),
    };

    fn save(&mut self) {
        self.amevcntr02_el0 = read_amevcntr02_el0();
        self.amevcntr03_el0 = read_amevcntr03_el0();
    }

    fn restore(&self) {
        write_amevcntr02_el0(self.amevcntr02_el0);
        write_amevcntr03_el0(self.amevcntr03_el0);
    }
}

struct AmuPowerdownContext {
    amcr_el0: AmcrEl0,
    amuserenr_el0: AmuserenrEl0,
    amevcntr00_el0: Amevcntr00El0,
    amevcntr01_el0: Amevcntr01El0,
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

impl AmuPowerdownContext {
    const EMPTY: Self = Self {
        amcr_el0: AmcrEl0::empty(),
        amuserenr_el0: AmuserenrEl0::empty(),
        amevcntr00_el0: Amevcntr00El0::empty(),
        amevcntr01_el0: Amevcntr01El0::empty(),
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
        self.amevcntr00_el0 = read_amevcntr00_el0();
        self.amevcntr01_el0 = read_amevcntr01_el0();

        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() {
            let implemented = read_amcg1idr_el0();

            if implemented.contains(Amcg1idrEl0::AMEVCNTR10_EL0) {
                self.amevcntr10_el0 = read_amevcntr10_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR11_EL0) {
                self.amevcntr11_el0 = read_amevcntr11_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR12_EL0) {
                self.amevcntr12_el0 = read_amevcntr12_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR13_EL0) {
                self.amevcntr13_el0 = read_amevcntr13_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR14_EL0) {
                self.amevcntr14_el0 = read_amevcntr14_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR15_EL0) {
                self.amevcntr15_el0 = read_amevcntr15_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR16_EL0) {
                self.amevcntr16_el0 = read_amevcntr16_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR17_EL0) {
                self.amevcntr17_el0 = read_amevcntr17_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR18_EL0) {
                self.amevcntr18_el0 = read_amevcntr18_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR19_EL0) {
                self.amevcntr19_el0 = read_amevcntr19_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR110_EL0) {
                self.amevcntr110_el0 = read_amevcntr110_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR111_EL0) {
                self.amevcntr111_el0 = read_amevcntr111_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR112_EL0) {
                self.amevcntr112_el0 = read_amevcntr112_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR113_EL0) {
                self.amevcntr113_el0 = read_amevcntr113_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR114_EL0) {
                self.amevcntr114_el0 = read_amevcntr114_el0();
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR115_EL0) {
                self.amevcntr115_el0 = read_amevcntr115_el0();
            }
        }
    }

    fn restore(&self) {
        write_amcr_el0(self.amcr_el0);
        write_amuserenr_el0(self.amuserenr_el0);
        write_amevcntr00_el0(self.amevcntr00_el0);
        write_amevcntr01_el0(self.amevcntr01_el0);

        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() {
            let implemented = read_amcg1idr_el0();

            if implemented.contains(Amcg1idrEl0::AMEVCNTR10_EL0) {
                write_amevcntr10_el0(self.amevcntr10_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR11_EL0) {
                write_amevcntr11_el0(self.amevcntr11_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR12_EL0) {
                write_amevcntr12_el0(self.amevcntr12_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR13_EL0) {
                write_amevcntr13_el0(self.amevcntr13_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR14_EL0) {
                write_amevcntr14_el0(self.amevcntr14_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR15_EL0) {
                write_amevcntr15_el0(self.amevcntr15_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR16_EL0) {
                write_amevcntr16_el0(self.amevcntr16_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR17_EL0) {
                write_amevcntr17_el0(self.amevcntr17_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR18_EL0) {
                write_amevcntr18_el0(self.amevcntr18_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR19_EL0) {
                write_amevcntr19_el0(self.amevcntr19_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR110_EL0) {
                write_amevcntr110_el0(self.amevcntr110_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR111_EL0) {
                write_amevcntr111_el0(self.amevcntr111_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR112_EL0) {
                write_amevcntr112_el0(self.amevcntr112_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR113_EL0) {
                write_amevcntr113_el0(self.amevcntr113_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR114_EL0) {
                write_amevcntr114_el0(self.amevcntr114_el0);
            }
            if implemented.contains(Amcg1idrEl0::AMEVCNTR115_EL0) {
                write_amevcntr115_el0(self.amevcntr115_el0);
            }
        }
    }
}

#[percore]
static AMU_POWERDOWN_CONTEXT: ExceptionLock<RefCell<AmuPowerdownContext>> =
    ExceptionLock::new(RefCell::new(AmuPowerdownContext::EMPTY));

#[percore]
static AMU_WORLD_CONTEXT: ExceptionLock<RefCell<PerWorld<AmuWorldContext>>> = ExceptionLock::new(
    RefCell::new(PerWorld([AmuWorldContext::EMPTY; CPU_DATA_CONTEXT_NUM])),
);

/// Activity Monitor Unit (AMU) extension support.
pub struct Amu<const CORE_COUNT: usize, CoresImpl: Cores> {
    selected_group1_counters: [Amcntenset1El0; CORE_COUNT],
    restrict_group1_access_to_el3: bool,
    _phantom: PhantomData<CoresImpl>,
}

impl<const CORE_COUNT: usize, CoresImpl: Cores> Amu<CORE_COUNT, CoresImpl> {
    /// Constructs a new instance of the AMU CPU extension.
    pub const fn new(
        selected_group1_counters: [Amcntenset1El0; CORE_COUNT],
        restrict_group1_access_to_el3: bool,
    ) -> Self {
        Self {
            selected_group1_counters,
            restrict_group1_access_to_el3,
            _phantom: PhantomData,
        }
    }

    fn enable_group0_counters(counters: Amcntenset0El0) {
        write_amcntenset0_el0(counters);
        isb();
    }

    fn disable_group0_counters(counters: Amcntenclr0El0) {
        write_amcntenclr0_el0(counters);
        isb();
    }

    fn enable_group1_counters(counters: Amcntenset1El0) {
        write_amcntenset1_el0(counters);
        isb();
    }

    fn disable_group1_counters(counters: Amcntenclr1El0) {
        write_amcntenclr1_el0(counters);
        isb();
    }

    fn group1_counter_is_implemented() -> bool {
        read_amcgcr_el0().cg1nc() > 0
    }
}

impl<const CORE_COUNT: usize, CoresImpl: Cores> Default for Amu<CORE_COUNT, CoresImpl> {
    fn default() -> Self {
        Self::new([Amcntenset1El0::empty(); CORE_COUNT], true)
    }
}

impl<const CORE_COUNT: usize, CoresImpl: Cores + Send + Sync> CpuExtension
    for Amu<CORE_COUNT, CoresImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64pfr0_el1().is_feat_amuv1_present()
    }

    fn init(&self) {
        write_amcr_el0(AmcrEl0::empty());

        // Enable always on counters (0-1), contexted counters (2-3) will be enabled at EL3 exit
        Self::enable_group0_counters(GROUP0_ALWAYS_ON);

        // Group 1 counters are defined by AMUv1, i.e. they are available without AMUv1p1 as well.
        // However, without AMUv1p1, the restricting of group 1 access to EL3 cannot be enforced.
        // This can result in a potentially insecure situation, so let's require AMUv1p1 for
        // enabling group 1 counters.
        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() {
            if self.restrict_group1_access_to_el3 {
                write_amcr_el0(AmcrEl0::CG1RZ);
            }

            if Self::group1_counter_is_implemented() {
                // Enable group 1 counters as defined by the platform
                let core_index = CoresImpl::core_index();
                Self::enable_group1_counters(self.selected_group1_counters[core_index]);
            }
        }
    }

    fn configure_per_world(&self, world: World, context: &mut PerWorldContext) {
        if world == World::NonSecure {
            // NS world can always access AMUv1 registers.
            context.cptr_el3 -= CptrEl3::TAM;

            if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() {
                // Enable access to AMUv1p1 virtual offset registers at lower ELs.
                context.scr_el3 |= ScrEl3::AMVOFFEN;
            }
        }
    }

    fn save_context(&self, world: World) {
        if !self.is_present() {
            return;
        }

        // The contexted counters were already disabled at EL3 entry
        exception_free(|token| {
            let mut context = AMU_WORLD_CONTEXT.get().borrow_mut(token);
            context[world].save();
        });
    }

    fn restore_context(&self, world: World) {
        if !self.is_present() {
            return;
        }

        // The contexted counters will be enabled at EL3 exit
        exception_free(|token| {
            let context = AMU_WORLD_CONTEXT.get().borrow(token).borrow();
            context[world].restore();
        });
    }

    fn save_context_before_suspend_to_powerdown(&self) {
        if !self.is_present() {
            return;
        }

        Self::disable_group0_counters(GROUP0_ALL);

        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() && Self::group1_counter_is_implemented()
        {
            Self::disable_group1_counters(GROUP1_ALL);
        }

        exception_free(|token| {
            let mut ctx = AMU_POWERDOWN_CONTEXT.get().borrow_mut(token);
            ctx.save();
        });

        // We need to update the saved value of contexted counters before suspend to powerdown
        self.save_context(World::NonSecure);
    }

    fn restore_context_after_suspend_to_powerdown(&self) {
        if !self.is_present() {
            return;
        }

        Self::disable_group0_counters(GROUP0_ALL);

        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() && Self::group1_counter_is_implemented()
        {
            Self::disable_group1_counters(GROUP1_ALL);
        }

        // The contexted counters will be restored later by the world context restore, so we don't
        // need to handle those here
        exception_free(|token| {
            let ctx = AMU_POWERDOWN_CONTEXT.get().borrow(token).borrow();
            ctx.restore();
        });

        Self::enable_group0_counters(GROUP0_ALWAYS_ON);

        if read_id_aa64pfr0_el1().is_feat_amuv1p1_present() && Self::group1_counter_is_implemented()
        {
            let core_index = CoresImpl::core_index();
            Self::enable_group1_counters(self.selected_group1_counters[core_index]);
        }

        isb();
    }
}
