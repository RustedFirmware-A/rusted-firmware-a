// Copyright (c) 2024, Google LLC. All rights reserved.
// Copyright (c) 2025, Arm Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

mod power_domain_tree;

use arm_psci::{
    AffinityInfo, Cookie, EntryPoint, ErrorCode, HwState, MemProtectRange, Mpidr, PowerState,
    ResetType, SystemOff2Type,
};
use bitflags::bitflags;
use core::fmt::{Debug, Formatter};
use percore::Cores;

use super::{owns, Service};
use crate::{
    aarch64::{dsb_sy, wfi},
    context::World,
    platform::{PlatformImpl, PlatformPowerState, PsciPlatformImpl},
    smccc::{FunctionId as OtherFunctionId, OwningEntityNumber, SmcReturn},
    sysregs::read_isr_el1,
};
use power_domain_tree::{AncestorPowerDomains, CpuPowerNode, PowerDomainTree};

bitflags! {
    /// Optional platform feature flags
    #[derive(Debug, Eq, PartialEq, Clone, Copy)]
    #[repr(transparent)]
    pub struct PsciPlatformOptionalFeatures: u64 {
        const SYSTEM_OFF2 = 1 << 0;
        const SYSTEM_RESET2 = 1 << 1;
        const MEM_PROTECT = 1 << 2;
        const MEM_PROTECT_CHECK_RANGE = 1 << 3;
        const CPU_FREEZE = 1 << 4;
        const CPU_DEFAULT_SUSPEND = 1 << 5;
        const NODE_HW_STATE = 1 << 6;
        const SYSTEM_SUSPEND = 1 << 7;
    }
}

/// Platform-specific power state interface
///
/// The platform has to provide a platform-specific power state type which implements this trait
/// and all of the dependent traits.
///
/// The type has to implement the `Ord` trait in a way the states are in ascending order from
/// running state to power down state.
pub trait PlatformPowerStateInterface:
    Debug + Clone + Copy + PartialEq + Ord + Into<usize>
{
    const OFF: Self;
    const RUN: Self;

    /// Returns the type of the platform-specific power state.
    fn power_state_type(&self) -> PowerStateType;
}

/// PSCI platform interface
///
/// The interface contains mandatory and optional constants and functions. Whether the platform
/// implements the optional functions has to be in sync with the reported optional features in the
/// `FEATURES` constant.
pub trait PsciPlatformInterface {
    /// Count of all power domains
    const POWER_DOMAIN_COUNT: usize;
    /// Maximal power level in the system
    const MAX_POWER_LEVEL: usize;

    /// Flags for describing optional features implemented by the platform.
    const FEATURES: PsciPlatformOptionalFeatures;

    /// Platform-specific power state type
    type PlatformPowerState: PlatformPowerStateInterface;

    /// Returns the power domain topology as the count of child nodes in a BFS traversal order.
    fn topology() -> &'static [usize];

    /// Tries to convert MPIDR to CPU index
    fn try_get_cpu_index_by_mpidr(mpidr: &Mpidr) -> Option<usize>;

    /// Tries to convert extended PSCI power state value into `PsciCompositePowerState`.
    fn try_parse_power_state(power_state: PowerState) -> Option<PsciCompositePowerState>;

    /// Places the current CPU into standby state and continues execution on interrupt.
    /// The caller has to guarantee that `cpu_state` is a standby power state, otherwise
    /// `cpu_standby` should panic.
    fn cpu_standby(&self, cpu_state: PlatformPowerState);

    /// Performs the necessary actions to turn off this cpu e.g. program the power controller.
    fn power_domain_suspend(&self, target_state: &PsciCompositePowerState);

    /// Performs platform-specific operations after a wake-up from standby/retention states.
    fn power_domain_suspend_finish(&self, target_state: &PsciCompositePowerState);

    /// Callback for platform housekeeping before turning off the CPU, optional.
    fn power_domain_off_early(
        &self,
        _target_state: &PsciCompositePowerState,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }

    /// Perform platform-specific actions to turn this cpu off e.g. program the power controller.
    fn power_domain_off(&self, target_state: &PsciCompositePowerState);

    /// Platform-specific function for entering WFI on power down, optional.
    fn power_domain_power_down_wfi(&self, _target_state: &PsciCompositePowerState) -> ! {
        dsb_sy();
        loop {
            wfi();
        }
    }

    /// Turn on power domain, which is identified by its MPIDR.
    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode>;

    /// Perform platform-specific actions after the CPU has been turned on.
    fn power_domain_on_finish(&self, target_state: &PsciCompositePowerState);

    /// Shuts down the system.
    fn system_off(&self) -> !;

    /// Suspend to disk, optional.
    fn system_off2(&self, _off_type: SystemOff2Type, _cookie: Cookie) -> Result<(), ErrorCode> {
        unimplemented!("SYSTEM_OFF2 is not implemented for the platform")
    }

    /// Resets the system, the behavior is equivalent to a hardware power-cycle sequence.
    fn system_reset(&self) -> !;

    /// Architectural or vendor specific reset function, optional.
    fn system_reset2(&self, _reset_type: ResetType, _cookie: Cookie) -> Result<(), ErrorCode> {
        unimplemented!("SYSTEM_RESET2 is not implemented for the platform")
    }

    /// Enable memory protection and returns the previous state, optional.
    fn mem_protect(&self, _enabled: bool) -> Result<bool, ErrorCode> {
        unimplemented!("MEM_PROTECT is not implemented for the platform")
    }

    /// Checks if the memory range is protected, optional.
    fn mem_protect_check_range(&self, _range: MemProtectRange) -> Result<(), ErrorCode> {
        unimplemented!("MEM_PROTECT_CHECK_RANGE is not implemented for the platform")
    }

    /// Places a core into an implementation defined low-power state where an interrupt does not
    /// return the core back into a running state.
    fn cpu_freeze(&self) -> ! {
        unimplemented!("CPU_FREEZE is not implemented for the platform")
    }

    /// Returns the power state for `CPU_DEFAULT_SUSPEND`, optional.
    fn cpu_default_suspend_power_state(&self) -> PowerState {
        unimplemented!("CPU_DEFAULT_SUSPEND is not implemented for the platform")
    }

    /// Returns the true hardware state of a power domain, optional.
    fn node_hw_state(&self, _mpidr: Mpidr, _power_level: u32) -> Result<HwState, ErrorCode> {
        unimplemented!("NODE_HW_STATE is not implemented for the platform")
    }

    /// Returns the power state for `SYSTEM_SUSPEND`, optional.
    fn sys_suspend_power_state(&self) -> PsciCompositePowerState {
        unimplemented!("SYSTEM_SUSPEND is not implemented for the platform")
    }

    /// Validates a non-secure entry point, optional.
    fn is_valid_ns_entrypoint(&self, _entry: &EntryPoint) -> bool {
        true
    }

    /// Checks if the CPU has pending interrupts
    fn has_pending_interrupts(&self) -> bool {
        read_isr_el1() != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerStateType {
    PowerDown,
    StandbyOrRetention,
    Run,
}

/// Object for storing platform-specific power state for multiple power levels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PsciCompositePowerState {
    pub states: [PlatformPowerState; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
}

impl PsciCompositePowerState {
    pub const CPU_POWER_LEVEL: usize = 0;

    /// States set to OFF on all levels.
    pub const OFF: Self = Self {
        states: [PlatformPowerState::OFF; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
    };

    /// States set to RUN on all levels.
    pub const RUN: Self = Self {
        states: [PlatformPowerState::RUN; PsciPlatformImpl::MAX_POWER_LEVEL + 1],
    };

    pub fn new(states: [PlatformPowerState; PsciPlatformImpl::MAX_POWER_LEVEL + 1]) -> Self {
        Self { states }
    }

    /// Returns the power state of the CPU level.
    pub fn cpu_level_state(&self) -> PlatformPowerState {
        self.states[Self::CPU_POWER_LEVEL]
    }

    /// Returns the power state of the highest level of the topology.
    pub fn highest_level_state(&self) -> PlatformPowerState {
        self.states[PsciPlatformImpl::MAX_POWER_LEVEL]
    }

    /// Find the highest power level which is not set to running state.
    pub fn find_highest_non_run_level(&self) -> Option<usize> {
        self.states
            .iter()
            .rposition(|state| state.power_state_type() != PowerStateType::Run)
    }

    /// Find the highest power level which is set to power down state.
    pub fn find_highest_power_down_level(&self) -> Option<usize> {
        self.states
            .iter()
            .rposition(|state| state.power_state_type() == PowerStateType::PowerDown)
    }

    /// Fill the structure with the current local states of the given CPU node and its ancestor
    /// non-CPU power domain nodes.
    pub fn set_local_states_from_nodes(
        &mut self,
        cpu: &CpuPowerNode,
        ancestors: &AncestorPowerDomains,
    ) {
        self.states[PsciCompositePowerState::CPU_POWER_LEVEL] = cpu.local_state();

        for (node, state) in ancestors
            .iter()
            .zip(&mut self.states[PsciCompositePowerState::CPU_POWER_LEVEL + 1..])
        {
            *state = node.local_state();
        }
    }

    /// Requests the power state for all ancestor nodes and sets the minimal local state for each
    /// node. When a CPU node enters a lower power state, its ancestor nodes may also be able to
    /// transition to a lower power state. Each non-CPU power node maintains a list of power states
    /// requested by its descendant nodes. This function sets the lowest power state permitted by
    /// the list of requested states.
    pub fn coordinate_state(&mut self, cpu_index: usize, ancestors: &mut AncestorPowerDomains) {
        let mut higher_levels_are_run = false;

        for (node, state) in ancestors
            .iter_mut()
            .zip(&mut self.states[PsciCompositePowerState::CPU_POWER_LEVEL + 1..])
        {
            node.set_requested_power_state(cpu_index, *state);

            if !higher_levels_are_run {
                node.set_minimal_allowed_state();
                *state = node.local_state();

                if state.power_state_type() == PowerStateType::Run {
                    // We reached a level where running states is required, so all power states
                    // on the higher level can be set to run.
                    higher_levels_are_run = true;
                }
            } else {
                // If there was a running state on a previous level, there's no need for finding
                // the minimal allowed state because it can only be in running state.
                *state = PlatformPowerState::RUN;
            }
        }
    }

    /// Checks that the composite state does not violate any PSCI rules.
    pub fn is_valid_suspend_request(&self, is_power_down_state: bool) -> bool {
        // There should be a non-run level
        if self.find_highest_non_run_level().is_none() {
            return false;
        };

        // Higher levels must be in less than or equal power state
        if !self.states.is_sorted_by(|a, b| a >= b) {
            return false;
        }

        if is_power_down_state {
            // There must be a power down state
            self.find_highest_power_down_level().is_some()
        } else {
            // Retention state, there should not be a power state on any level
            self.find_highest_power_down_level().is_none()
        }
    }
}

/// Main PSCI structure of the PSCI implementation that handles all the PSCI calls and stores the
/// the power state representation of each power domain.
pub struct Psci {
    platform: PsciPlatformImpl,
    power_domain_tree: PowerDomainTree,
}

impl Psci {
    pub fn new(platform: PsciPlatformImpl) -> Self {
        let power_domain_tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        {
            // Init primary CPU
            let cpu_index = Self::local_cpu_index();
            let mut cpu = power_domain_tree.locked_cpu_node(cpu_index);

            power_domain_tree.with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                cpu.set_affinity_info(AffinityInfo::On);
                cpu.set_local_state(PlatformPowerState::RUN);

                for node in ancestors.iter_mut() {
                    node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                    node.set_local_state(PlatformPowerState::RUN);
                }
            });
        }

        Self {
            platform,
            power_domain_tree,
        }
    }

    fn local_cpu_index() -> usize {
        PlatformImpl::core_index()
    }
}

const FUNCTION_NUMBER_MIN: u16 = 0x0000;
const FUNCTION_NUMBER_MAX: u16 = 0x001F;

impl Service for Psci {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_smc(
        _function: OtherFunctionId,
        _x1: u64,
        _x2: u64,
        _x3: u64,
        _x4: u64,
        _world: World,
    ) -> SmcReturn {
        let result = ErrorCode::NotSupported as u32;
        result.into()
    }
}

impl Debug for Psci {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        self.power_domain_tree.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::{PsciPlatformImpl, *};

    #[test]
    fn psci_composite_power_state() {
        let mut composite_state = PsciCompositePowerState::OFF;
        assert_eq!(PlatformPowerState::OFF, composite_state.cpu_level_state());

        assert_eq!(
            PlatformPowerState::OFF,
            composite_state.highest_level_state()
        );

        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::RUN;
        assert_eq!(PlatformPowerState::RUN, composite_state.cpu_level_state());

        composite_state = PsciCompositePowerState::OFF;
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL),
            composite_state.find_highest_power_down_level()
        );
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL),
            composite_state.find_highest_non_run_level()
        );

        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL] = PlatformPowerState::RUN;
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL - 1),
            composite_state.find_highest_power_down_level()
        );
        assert_eq!(
            Some(PsciPlatformImpl::MAX_POWER_LEVEL - 1),
            composite_state.find_highest_non_run_level()
        );

        composite_state = PsciCompositePowerState::RUN;
        assert_eq!(None, composite_state.find_highest_power_down_level());
        assert_eq!(None, composite_state.find_highest_non_run_level());

        composite_state = PsciCompositePowerState::RUN;
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::OFF;
        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::RUN;
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::OFF;
        assert!(composite_state.is_valid_suspend_request(true));
        assert!(!composite_state.is_valid_suspend_request(false));

        composite_state = PsciCompositePowerState::RUN;
        composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL] = PlatformPowerState::OFF;
        assert!(composite_state.is_valid_suspend_request(true));
        assert!(!composite_state.is_valid_suspend_request(false));
    }

    #[test]
    fn psci_composite_power_state_set_from_nodes() {
        let mut composite_state = PsciCompositePowerState::OFF;
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        let mut cpu = tree.locked_cpu_node(2);
        tree.with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
            cpu.set_local_state(PlatformPowerState::RUN);
            for ancestor in ancestors.iter_mut() {
                ancestor.set_local_state(PlatformPowerState::RUN);
            }

            composite_state.set_local_states_from_nodes(cpu, &ancestors);
        });

        assert_eq!(
            [PlatformPowerState::RUN; {
                PsciPlatformImpl::MAX_POWER_LEVEL - PsciCompositePowerState::CPU_POWER_LEVEL + 1
            }],
            composite_state.states[PsciCompositePowerState::CPU_POWER_LEVEL..]
        )
    }

    #[test]
    fn psci_composite_power_state_coordination() {
        let mut composite_state = PsciCompositePowerState::OFF;
        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL - 1] = PlatformPowerState::RUN;
        composite_state.states[PsciPlatformImpl::MAX_POWER_LEVEL] = PlatformPowerState::RUN;
        let tree = PowerDomainTree::new(PsciPlatformImpl::topology());

        let mut cpu = tree.locked_cpu_node(2);
        tree.with_ancestors_locked(&mut cpu, |_cpu, mut ancestors| {
            composite_state.coordinate_state(2, &mut ancestors);
        });
    }
}
