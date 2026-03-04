// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Runtime services which handle SMCs from lower ELs.

pub mod arch;
mod errata_management;
pub mod ffa;
pub mod psci;
#[cfg(feature = "rme")]
pub mod rmmd;
pub mod trng;

#[cfg(feature = "rme")]
use crate::services::rmmd::Rmmd;
use crate::{
    context::{CpuStateAccess, World, set_initial_world, switch_world},
    exceptions::{RunResult, enter_world, inject_undef64},
    gicv3::{self, InterruptType},
    platform::{
        PSCI_MAX_POWER_LEVEL, PSCI_STATE_COUNT, Platform, PlatformImpl, TRNG_REQ_WORDS,
        exception_free,
    },
    services::{
        arch::Arch,
        errata_management::ErrataManagement,
        ffa::spmd::Spmd,
        psci::{Psci, PsciPlatformInterface},
        trng::{Trng, TrngPlatformInterface, words_in_pool},
    },
    smccc::{FunctionId, NOT_SUPPORTED, SetFrom, SmcReturn},
};
use arm_sysregs::EsrEl3;
use log::debug;
use spin::Lazy;

/// Helper macro to define the range of SMC function ID values covered by a service
macro_rules! owns {
    // service handles the entire Owning Entity Number (OEN)
    ($owning_entity:expr) => {
        #[inline(always)]
        fn owns(&self, function: $crate::smccc::FunctionId) -> bool {
            function.oen() == $owning_entity
                && matches!(
                    function.call_type(),
                    $crate::smccc::SmcccCallType::Fast32 | $crate::smccc::SmcccCallType::Fast64
                )
        }
    };
    // service handles a sub-range of the OEN
    // range refers to the lower 16 bits [15:0] of the SMC FunctionId
    ($owning_entity:expr, $range:expr) => {
        #[inline(always)]
        fn owns(&self, function: $crate::smccc::FunctionId) -> bool {
            function.oen() == $owning_entity
                && $range.contains(&function.number())
                && matches!(
                    function.call_type(),
                    $crate::smccc::SmcccCallType::Fast32 | $crate::smccc::SmcccCallType::Fast64
                )
        }
    };
}
pub(crate) use owns;

/// A service which handles some range of SMC calls.
///
/// According to SMCCC v1.3+ the implementation must disregard the SVE hint bit in the function ID
/// and consider it to be 0 for the purpose of function identification.
pub trait Service {
    /// Returns whether this service is intended to handle the given function ID.
    fn owns(&self, function: FunctionId) -> bool;

    /// Handles the given SMC call from Normal World.
    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        regs.set_from(NOT_SUPPORTED);
        World::NonSecure
    }

    /// Handles the given SMC call from Secure World.
    fn handle_secure_smc(&self, regs: &mut SmcReturn) -> World {
        regs.set_from(NOT_SUPPORTED);
        World::Secure
    }

    /// Handles the given SMC call from Realm World.
    #[cfg(feature = "rme")]
    fn handle_realm_smc(&self, regs: &mut SmcReturn) -> World {
        regs.set_from(NOT_SUPPORTED);
        World::Realm
    }
}

const NON_CPU_DOMAIN_COUNT: usize =
    <PlatformImpl as Platform>::PsciPlatformImpl::POWER_DOMAIN_COUNT - PlatformImpl::CORE_COUNT;
const TRNG_WORDS_IN_POOL: usize = words_in_pool(TRNG_REQ_WORDS);
static SERVICES: Lazy<
    Services<
        { PlatformImpl::CORE_COUNT },
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        NON_CPU_DOMAIN_COUNT,
        TRNG_REQ_WORDS,
        TRNG_WORDS_IN_POOL,
        PlatformImpl,
    >,
> = Lazy::new(Services::new);

/// Contains an instance of all of the currently implemented services.
pub struct Services<
    const CORE_COUNT: usize,
    const PSCI_STATE_COUNT: usize,
    const PSCI_MAX_POWER_LEVEL: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    const TRNG_REQ_WORDS: usize,
    const TRNG_WORDS_IN_POOL: usize,
    PlatformImpl: CpuStateAccess + Platform + 'static,
> where
    <PlatformImpl as Platform>::PsciPlatformImpl: PsciPlatformInterface<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            CORE_COUNT,
            NON_CPU_DOMAIN_COUNT,
        >,
    <PlatformImpl as Platform>::TrngPlatformImpl: TrngPlatformInterface<TRNG_REQ_WORDS>,
{
    arch: Arch<PlatformImpl>,
    pub psci: Psci<
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        CORE_COUNT,
        NON_CPU_DOMAIN_COUNT,
        PlatformImpl,
        PlatformImpl::PsciPlatformImpl,
        Spmd<CORE_COUNT, PlatformImpl>,
    >,
    platform: PlatformImpl::PlatformServiceImpl,
    /// The FF-A SPMD service.
    pub spmd: Spmd<CORE_COUNT, PlatformImpl>,
    /// The CCA service for communication with TF-RMM.
    #[cfg(feature = "rme")]
    pub rmmd: Rmmd<CORE_COUNT, PlatformImpl>,
    trng: Trng<TRNG_REQ_WORDS, TRNG_WORDS_IN_POOL, PlatformImpl::TrngPlatformImpl>,
    errata_management: ErrataManagement<PlatformImpl>,
}

impl
    Services<
        { PlatformImpl::CORE_COUNT },
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        NON_CPU_DOMAIN_COUNT,
        TRNG_REQ_WORDS,
        TRNG_WORDS_IN_POOL,
        PlatformImpl,
    >
{
    /// Constructs a new instance of the services.
    fn new() -> Self {
        Self {
            arch: Arch::new(),
            psci: Psci::new(PlatformImpl::psci_platform().unwrap(), || &Self::get().spmd),
            platform: PlatformImpl::create_service(),
            spmd: Spmd::new(),
            #[cfg(feature = "rme")]
            rmmd: Rmmd::new(),
            trng: Trng::new(),
            errata_management: ErrataManagement::new(),
        }
    }

    /// Returns a reference to the global Services instance.
    ///
    /// Also initializes it if it hasn't been initialized yet.
    pub fn get() -> &'static Self {
        &SERVICES
    }
}

impl<
    const CORE_COUNT: usize,
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    const TRNG_REQ_WORDS: usize,
    const TRNG_WORDS_IN_POOL: usize,
    PlatformImpl: CpuStateAccess + Platform,
>
    Services<
        CORE_COUNT,
        STATE_COUNT,
        MAX_POWER_LEVEL,
        NON_CPU_DOMAIN_COUNT,
        TRNG_REQ_WORDS,
        TRNG_WORDS_IN_POOL,
        PlatformImpl,
    >
where
    <PlatformImpl as Platform>::PsciPlatformImpl:
        PsciPlatformInterface<STATE_COUNT, MAX_POWER_LEVEL, CORE_COUNT, NON_CPU_DOMAIN_COUNT>,
    <PlatformImpl as Platform>::TrngPlatformImpl: TrngPlatformInterface<TRNG_REQ_WORDS>,
{
    fn handle_smc(&self, regs: &mut SmcReturn, world: World) -> World {
        let function = FunctionId(regs.values()[0] as u32);

        if !function.valid() {
            regs.set_from(NOT_SUPPORTED);
            return world;
        }

        let service: &dyn Service = if self.arch.owns(function) {
            &self.arch
        } else if self.psci.owns(function) {
            &self.psci
        } else if self.platform.owns(function) {
            &self.platform
        } else if self.spmd.owns(function) {
            &self.spmd
        } else if self.errata_management.owns(function) {
            &self.errata_management
        } else if self.trng.owns(function) {
            &self.trng
        } else {
            #[cfg(feature = "rme")]
            if self.rmmd.owns(function) {
                &self.rmmd
            } else {
                regs.set_from(NOT_SUPPORTED);
                return world;
            }

            #[cfg(not(feature = "rme"))]
            {
                regs.set_from(NOT_SUPPORTED);
                return world;
            }
        };

        match world {
            World::NonSecure => service.handle_non_secure_smc(regs),
            World::Secure => service.handle_secure_smc(regs),
            #[cfg(feature = "rme")]
            World::Realm => service.handle_realm_smc(regs),
        }
    }

    fn handle_interrupt(&self, regs: &mut SmcReturn, world: World) -> World {
        let interrupt_type = gicv3::get_pending_interrupt_type();

        match (interrupt_type, world) {
            (InterruptType::Secure, World::NonSecure) => self.spmd.forward_secure_interrupt(regs),
            // TODO:
            // Group 0 interrupts hitting in SWd should be catched by the SPMC and passed to EL3
            // synchronously, by invoking FFA_EL3_INTR_HANDLE.
            (InterruptType::El3, World::Secure) => todo!(),
            (InterruptType::El3, World::NonSecure) => {
                gicv3::handle_group0_interrupt::<PlatformImpl>();
                regs.mark_empty();
                world
            }
            (InterruptType::Invalid, _) => {
                // If the interrupt controller reports a spurious interrupt then return to where we
                // came from.
                regs.mark_empty();
                world
            }
            _ => panic!(
                "Unsupported interrupt routing. Interrupt type: {interrupt_type:?} world: {world:?}"
            ),
        }
    }

    fn handle_sysreg_trap(&self, esr: EsrEl3, world: World) {
        // Default behaviour is to repeat the same instruction, unless the trap handler requests
        // stepping to the next one.
        #[allow(unused)]
        let mut step_to_next_instr = false;

        #[allow(clippy::match_single_binding)]
        match esr & EsrEl3::ISS_SYSREG_OPCODE_MASK {
            // TODO: add trap handlers, should set step_to_next_instr as necessary
            _ => {
                inject_undef64::<PlatformImpl>(world);
                return;
            }
        }

        #[allow(unreachable_code)]
        if step_to_next_instr {
            exception_free(|token| {
                PlatformImpl::cpu_state(token)[world].skip_lower_el_instruction();
            })
        }
    }

    fn per_world_loop(&self, regs: &mut SmcReturn, world: World) -> World {
        let mut next_world;

        loop {
            next_world = match enter_world::<PlatformImpl>(regs, world) {
                RunResult::Smc => self.handle_smc(regs, world),
                RunResult::Interrupt => self.handle_interrupt(regs, world),
                RunResult::SysregTrap { esr } => {
                    self.handle_sysreg_trap(esr, world);
                    regs.mark_empty();
                    world
                }
            };

            if next_world != world {
                break next_world;
            }
        }
    }

    /// The main runtime loop.
    ///
    /// This method is responsible for entering all worlds for the first time in the correct order.
    /// After that, it will continuously process the results from a lower EL when it has returned to
    /// EL3, switch to another world if necessary and enter a lower EL with the new arguments. The
    /// initial entry to each world must happen with `SmcReturn::EMPTY` argument, in order to avoid
    /// overwriting the contents of GP regs that have already been set by initialise_contexts() in
    /// `bl31_main()`. This method doesn't return, it should be called on each core as the last step
    /// of the boot process, i.e. after setting up MMU, GIC, etc.
    pub fn run_loop(&self) -> ! {
        let mut current_world = World::Secure;
        let mut regs = SmcReturn::EMPTY;

        debug!("Booting Secure World");
        set_initial_world::<PlatformImpl>(World::Secure);
        // TODO: implement separate boot loop for Secure World
        let next_world = self.per_world_loop(&mut regs, World::Secure);
        assert_eq!(next_world, World::NonSecure);

        #[cfg(feature = "rme")]
        {
            // If the RMM boot failed, do not try to boot Realm world again.
            if !self.rmmd.boot_failure() {
                debug!("Booting Realm World");
                switch_world::<PlatformImpl>(current_world, World::Realm);
                current_world = World::Realm;
                // TODO: implement separate boot loop for Realm World
                regs.mark_empty();
                let next_world = self.per_world_loop(&mut regs, World::Realm);
                assert_eq!(next_world, World::NonSecure);
            }
        }

        regs.mark_empty();
        let mut next_world = World::NonSecure;
        debug!("Booting Normal World");

        loop {
            switch_world::<PlatformImpl>(current_world, next_world);
            current_world = next_world;
            next_world = self.per_world_loop(&mut regs, current_world);
            assert_ne!(current_world, next_world);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::arch::{SMCCC_VERSION, SMCCC_VERSION_1_5},
        smccc::FunctionId,
    };

    /// Tests the SMCCC arch version call as a simple example of SMC dispatch.
    ///
    /// The point of this isn't to test every individual SMC call, just that the common code in
    /// `handle_smc` works. Individual SMC calls can be tested directly within their modules.
    #[test]
    fn handle_smc_arch_version() {
        let services = Services::new();

        let mut function = FunctionId(SMCCC_VERSION);

        // Set the SVE hint bit to test if the handler will can treat this correctly.
        function.set_sve_hint();

        let mut regs = SmcReturn::EMPTY;
        regs.set_from(function.0);

        let new_world = services.handle_smc(&mut regs, World::NonSecure);

        assert_eq!(new_world, World::NonSecure);
        assert_eq!(regs.values(), [SMCCC_VERSION_1_5 as u64]);
    }
}
