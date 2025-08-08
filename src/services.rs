// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod arch;
pub mod ffa;
pub mod psci;
#[cfg(feature = "rme")]
pub mod rmmd;

use crate::{
    context::{World, cpu_state, set_initial_world, switch_world},
    exceptions::{RunResult, enter_world, inject_undef64},
    gicv3::{self, InterruptType},
    platform::{self, Platform, PlatformImpl, exception_free},
    smccc::{FunctionId, NOT_SUPPORTED, SmcReturn},
    sysregs::Esr,
};
use log::info;
use spin::Lazy;

/// Helper macro to define the range of SMC function ID values covered by a service
#[macro_export]
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
    fn handle_non_secure_smc(&self, _regs: &[u64; 18]) -> (SmcReturn, World) {
        (NOT_SUPPORTED.into(), World::NonSecure)
    }

    /// Handles the given SMC call from Secure World.
    fn handle_secure_smc(&self, _regs: &[u64; 18]) -> (SmcReturn, World) {
        (NOT_SUPPORTED.into(), World::Secure)
    }

    /// Handles the given SMC call from Realm World.
    #[cfg(feature = "rme")]
    fn handle_realm_smc(&self, _regs: &[u64; 18]) -> (SmcReturn, World) {
        (NOT_SUPPORTED.into(), World::Realm)
    }
}

static SERVICES: Lazy<Services> = Lazy::new(Services::new);

/// Contains an instance of all of the currently implemented services.
pub struct Services {
    pub arch: arch::Arch,
    pub psci: psci::Psci,
    pub platform: platform::PlatformServiceImpl,
    pub spmd: ffa::Spmd,
    #[cfg(feature = "rme")]
    pub rmmd: rmmd::Rmmd,
}

impl Services {
    /// Returns a reference to the global Services instance.
    ///
    /// Also, initializes it if it hasn't been initialized yet.
    pub fn get() -> &'static Self {
        &SERVICES
    }

    fn new() -> Self {
        Self {
            arch: arch::Arch::new(),
            psci: psci::Psci::new(PlatformImpl::psci_platform().unwrap()),
            platform: PlatformImpl::create_service(),
            spmd: ffa::Spmd::new(),
            #[cfg(feature = "rme")]
            rmmd: rmmd::Rmmd::new(),
        }
    }

    fn handle_smc(&self, regs: &[u64; 18], world: World) -> (SmcReturn, World) {
        let function = FunctionId(regs[0] as u32);

        if !function.valid() {
            return (NOT_SUPPORTED.into(), world);
        }

        let service: &dyn Service = if self.arch.owns(function) {
            &self.arch
        } else if self.psci.owns(function) {
            &self.psci
        } else if self.platform.owns(function) {
            &self.platform
        } else if self.spmd.owns(function) {
            &self.spmd
        } else {
            #[cfg(feature = "rme")]
            if self.rmmd.owns(function) {
                &self.rmmd
            } else {
                return (NOT_SUPPORTED.into(), world);
            }

            #[cfg(not(feature = "rme"))]
            return (NOT_SUPPORTED.into(), world);
        };

        let (out_regs, next_world) = match world {
            World::NonSecure => service.handle_non_secure_smc(regs),
            World::Secure => service.handle_secure_smc(regs),
            #[cfg(feature = "rme")]
            World::Realm => service.handle_realm_smc(regs),
        };

        (out_regs, next_world)
    }

    fn handle_interrupt(&self, world: World) -> (SmcReturn, World) {
        let interrupt_type = gicv3::get_pending_interrupt_type();

        match (interrupt_type, world) {
            (InterruptType::Secure, World::NonSecure) => self.spmd.forward_secure_interrupt(),
            // TODO:
            // Group 0 interrupts hitting in SWd should be catched by the SPMC and passed to EL3
            // synchronously, by invoking FFA_EL3_INTR_HANDLE.
            (InterruptType::El3, World::Secure) => todo!(),
            (InterruptType::El3, World::NonSecure) => {
                gicv3::handle_group0_interrupt();
                (SmcReturn::EMPTY, world)
            }
            (InterruptType::Invalid, _) => {
                // If the interrupt controller reports a spurious interrupt then return to where we
                // came from.
                (SmcReturn::EMPTY, world)
            }
            _ => panic!(
                "Unsupported interrupt routing. Interrupt type: {interrupt_type:?} world: {world:?}"
            ),
        }
    }

    fn handle_sysreg_trap(&self, esr: Esr, world: World) {
        // Default behaviour is to repeat the same instruction, unless the trap handler requests
        // stepping to the next one.
        #[allow(unused)]
        let mut step_to_next_instr = false;

        #[allow(clippy::match_single_binding)]
        match esr & Esr::ISS_SYSREG_OPCODE_MASK {
            // TODO: add trap handlers, should set step_to_next_instr as necessary
            _ => {
                inject_undef64(world);
                return;
            }
        }

        #[allow(unreachable_code)]
        if step_to_next_instr {
            exception_free(|token| {
                cpu_state(token)
                    .context_mut(world)
                    .skip_lower_el_instruction();
            })
        }
    }

    fn per_world_loop(&self, mut regs: SmcReturn, world: World) -> (SmcReturn, World) {
        let mut next_world;

        loop {
            (regs, next_world) = match enter_world(&regs, world) {
                RunResult::Smc { regs } => self.handle_smc(&regs, world),
                RunResult::Interrupt => self.handle_interrupt(world),
                RunResult::SysregTrap { esr } => {
                    self.handle_sysreg_trap(esr, world);
                    (SmcReturn::EMPTY, world)
                }
            };

            if next_world != world {
                break (regs, next_world);
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

        info!("Booting Secure World");
        set_initial_world(World::Secure);
        // TODO: implement separate boot loop for Secure World
        let (_, next_world) = self.per_world_loop(SmcReturn::EMPTY, World::Secure);
        assert_eq!(next_world, World::NonSecure);

        #[cfg(feature = "rme")]
        {
            info!("Booting Realm World");
            switch_world(current_world, World::Realm);
            current_world = World::Realm;
            // TODO: implement separate boot loop for Realm World
            let (_, next_world) = self.per_world_loop(SmcReturn::EMPTY, World::Realm);
            assert_eq!(next_world, World::NonSecure);
        }

        let mut regs = SmcReturn::EMPTY;
        let mut next_world = World::NonSecure;
        info!("Booting Normal World");

        loop {
            switch_world(current_world, next_world);
            current_world = next_world;
            (regs, next_world) = self.per_world_loop(regs, current_world);
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
        let mut regs = [0u64; 18];

        let mut function = FunctionId(SMCCC_VERSION);

        // Set the SVE hint bit to test if the handler will can treat this correctly.
        function.set_sve_hint();
        regs[0] = function.0.into();

        let (result, new_world) = services.handle_smc(&regs, World::NonSecure);

        assert_eq!(new_world, World::NonSecure);
        assert_eq!(result.values(), [SMCCC_VERSION_1_5 as u64]);
    }
}
