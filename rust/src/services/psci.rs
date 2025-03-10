// Copyright (c) 2024, Google LLC. All rights reserved.
// Copyright (c) 2025, Arm Ltd. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause

mod power_domain_tree;
mod spmd_stub;

use arm_psci::{
    AffinityInfo, Cookie, EntryPoint, ErrorCode, Function, HwState, MemProtectRange, Mpidr,
    PowerState, ResetType, ReturnCode, SystemOff2Type,
};
use bitflags::bitflags;
use core::fmt::{Debug, Formatter};
use percore::Cores;
use power_domain_tree::{AncestorPowerDomains, CpuPowerNode, PowerDomainTree};
use spmd_stub::SPMD;

use crate::{
    aarch64::{dsb_sy, wfi},
    context::World,
    platform::{PlatformImpl, PlatformPowerState, PsciPlatformImpl},
    smccc::{FunctionId as OtherFunctionId, OwningEntityNumber, SmcReturn},
    sysregs::read_isr_el1,
};

use super::{owns, Service};

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

    /// Handles `CPU_SUSPEND` PSCI call by following the steps below.
    /// * If the a standby power state is requested which only affects the CPU level, the wait for
    ///   interrupts by calling `cpu_standby` and then return after an interrupt.
    /// * If a power down state is requested or a standby request affects higher levels, then call
    ///   `cpu_suspend_start`.
    fn cpu_suspend(
        &self,
        power_state: PowerState,
        entry_point: EntryPoint,
    ) -> Result<(), ErrorCode> {
        let cpu_index = Self::local_cpu_index();
        let composite_state: PsciCompositePowerState =
            PsciPlatformImpl::try_parse_power_state(power_state)
                .ok_or(ErrorCode::InvalidParameters)?;

        let is_power_down_state = matches!(power_state, PowerState::PowerDown(_));

        assert!(composite_state.is_valid_suspend_request(is_power_down_state));

        let highest_affected_level = composite_state
            .find_highest_non_run_level()
            .expect("Invalid target power level for suspend operation");

        if !is_power_down_state
            && highest_affected_level == PsciCompositePowerState::CPU_POWER_LEVEL
        {
            // CPU standby which does not affect parent nodes
            let cpu_pd_state = composite_state.cpu_level_state();
            self.power_domain_tree
                .locked_cpu_node(cpu_index)
                .set_local_state(cpu_pd_state);

            // Start waiting for interrupts.
            self.platform.cpu_standby(cpu_pd_state);
            // Continue execution after an interrupt woke up the CPU.

            self.power_domain_tree
                .locked_cpu_node(cpu_index)
                .set_local_state(PlatformPowerState::RUN);

            Ok(())
        } else {
            if is_power_down_state && !self.platform.is_valid_ns_entrypoint(&entry_point) {
                return Err(ErrorCode::InvalidAddress);
            }

            self.cpu_suspend_start(
                cpu_index,
                Some(power_state),
                entry_point,
                highest_affected_level,
                composite_state,
                is_power_down_state,
            )
        }
    }

    /// Handles the common part of `CPU_SUSPEND` and `SYSTEM_SUSPEND` PSCI calls. The `power_state`
    /// argument is `None` when coming from `SYSTEM_SUSPEND` handler because it does not have
    /// power state parameter.
    ///
    /// The function follows the steps below.
    /// * Return immediately if there's a pending interrupt.
    /// * Otherwise determine the valid state for each level without violating any power domain
    ///   rules.
    /// * Request this power state from the platform layer (`power_domain_suspend`). This step does
    ///   not trigger an immediate shutdown of the power domain.
    /// * Power down the domain by calling `power_domain_power_down_wfi` if this is a power down
    ///   request. Normally this function calls `WFI` in a loop which actually turns off the domain
    ///   but platforms may diverge from this. `cpu_suspend_start` does not return after this point.
    ///   When the CPU wakes up, the boot code must call `handle_cpu_boot` that completes the power
    ///   down suspend operation.
    /// * If the requested power state is a standby state, call a `WFI` and restore running state
    ///   after waking up by an interrupt.
    fn cpu_suspend_start(
        &self,
        cpu_index: usize,
        power_state: Option<PowerState>,
        entry: EntryPoint,
        highest_affected_level: usize,
        mut composite_state: PsciCompositePowerState,
        is_power_down_state: bool,
    ) -> Result<(), ErrorCode> {
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let has_pending_interrupt = self.power_domain_tree.with_ancestors_locked_to_max_level(
            &mut cpu,
            highest_affected_level,
            |cpu, mut ancestors| {
                if self.platform.has_pending_interrupts() {
                    return true;
                }

                composite_state.coordinate_state(cpu_index, &mut ancestors);
                cpu.set_local_state(composite_state.cpu_level_state());

                if is_power_down_state {
                    if let Some(state) = power_state {
                        self.notify_spmd(Function::CpuSuspend { state, entry });
                    } else {
                        self.notify_spmd(Function::SystemSuspend { entry });
                    }
                    cpu.set_entry_point(entry);
                }

                self.platform.power_domain_suspend(&composite_state);
                false
            },
        );
        drop(cpu); // Unlock CPU before entering suspend state

        if has_pending_interrupt {
            // Has pending interrupts, do not suspend
            return Ok(());
        }

        // Continue suspend operation
        if is_power_down_state {
            self.platform.power_domain_power_down_wfi(&composite_state);
            // This branch does not return
        } else {
            // Go to suspend by waiting for interrupts.
            wfi();

            // Restore running state after wake-up.
            let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
            self.power_domain_tree
                .with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                    composite_state.set_local_states_from_nodes(cpu, &ancestors);

                    self.platform.power_domain_suspend_finish(&composite_state);
                    cpu.clear_highest_affected_level();
                    cpu.set_local_state(PlatformPowerState::RUN);

                    for node in ancestors.iter_mut() {
                        node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                        node.set_local_state(PlatformPowerState::RUN);
                    }
                });
            Ok(())
        }
    }

    /// Handles `CPU_OFF` PSCI call.
    /// On success, turns off the current CPU and does not return.
    fn cpu_off(&self) -> Result<(), ErrorCode> {
        let cpu_index = Self::local_cpu_index();
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let mut composite_state = PsciCompositePowerState::OFF;

        self.platform.power_domain_off_early(&composite_state)?;

        self.power_domain_tree
            .with_ancestors_locked(&mut cpu, |cpu, mut ancestors| {
                self.notify_spmd(Function::CpuOff);
                cpu.set_local_state(PlatformPowerState::OFF);
                composite_state.coordinate_state(cpu_index, &mut ancestors);
                self.platform.power_domain_off(&composite_state);
            });

        cpu.set_affinity_info(AffinityInfo::Off);

        // Unlock CPU before actually turning it off
        drop(cpu);

        self.platform.power_domain_power_down_wfi(&composite_state);
        // Does not return
    }

    /// Handles `CPU_ON` PSCI call by turning on the CPU identified by the given `target_cpu` MPIDR.
    /// The caller has to provide a valid non-secure entry point for the CPU.
    fn cpu_on(&self, target_cpu: Mpidr, entry: EntryPoint) -> Result<(), ErrorCode> {
        let cpu_index = PsciPlatformImpl::try_get_cpu_index_by_mpidr(&target_cpu)
            .ok_or(ErrorCode::InvalidParameters)?;

        if !self.platform.is_valid_ns_entrypoint(&entry) {
            return Err(ErrorCode::InvalidAddress);
        }

        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        match cpu.affinity_info() {
            AffinityInfo::On => return Err(ErrorCode::AlreadyOn),
            AffinityInfo::OnPending => return Err(ErrorCode::OnPending),
            // The CPU was off, so continue CPU on operation.
            AffinityInfo::Off => {}
        }

        self.notify_spmd(Function::CpuOn { target_cpu, entry });

        cpu.set_affinity_info(AffinityInfo::OnPending);

        match self.platform.power_domain_on(target_cpu) {
            Ok(_) => {
                cpu.set_entry_point(entry);
                Ok(())
            }
            Err(error) => {
                cpu.set_affinity_info(AffinityInfo::Off);
                Err(error)
            }
        }
    }

    /// This function must be called when a CPU is powered up. It returns the non-secure entry
    /// point.
    pub fn handle_cpu_boot(&self) -> EntryPoint {
        let cpu_index = Self::local_cpu_index();
        let mut cpu = self.power_domain_tree.locked_cpu_node(cpu_index);
        let mut composite_state = PsciCompositePowerState::RUN;

        let affinity_info = cpu.affinity_info();
        if affinity_info == AffinityInfo::Off {
            drop(cpu);
            panic!("Unexpected affinity info state");
        }

        let target_power_level = cpu
            .highest_affected_level()
            .unwrap_or(PsciPlatformImpl::MAX_POWER_LEVEL);

        self.power_domain_tree.with_ancestors_locked_to_max_level(
            &mut cpu,
            target_power_level,
            |cpu, mut ancestors| {
                composite_state.set_local_states_from_nodes(cpu, &ancestors);

                if affinity_info == AffinityInfo::OnPending {
                    // Finishing CPU_ON
                    self.platform.power_domain_on_finish(&composite_state);

                    SPMD.handle_cold_boot();

                    cpu.set_affinity_info(AffinityInfo::On);
                } else {
                    // Waking up from suspend
                    assert_eq!(affinity_info, AffinityInfo::On);
                    assert_eq!(
                        composite_state.cpu_level_state().power_state_type(),
                        PowerStateType::PowerDown
                    );

                    SPMD.handle_warm_boot();

                    self.platform.power_domain_suspend_finish(&composite_state);
                    cpu.clear_highest_affected_level();
                }

                cpu.set_local_state(PlatformPowerState::RUN);

                for node in ancestors.iter_mut() {
                    node.set_requested_power_state(cpu_index, PlatformPowerState::RUN);
                    node.set_local_state(PlatformPowerState::RUN);
                }
            },
        );

        let entry_point = cpu.pop_entry_point();
        drop(cpu); // Unlock before possible panic

        entry_point.expect("entry point not set for booting CPU")
    }

    /// Handles `AFFINITY_INFO` PSCI call.
    fn affinity_info(
        &self,
        target_affinity: Mpidr,
        lowest_affinity_level: u32,
    ) -> Result<AffinityInfo, ErrorCode> {
        let cpu_index = PsciPlatformImpl::try_get_cpu_index_by_mpidr(&target_affinity)
            .ok_or(ErrorCode::InvalidParameters)?;

        if lowest_affinity_level as usize > PsciCompositePowerState::CPU_POWER_LEVEL {
            // We don't support levels higher than CPU_POWER_LEVEL.
            return Err(ErrorCode::InvalidParameters);
        }

        Ok(self
            .power_domain_tree
            .locked_cpu_node(cpu_index)
            .affinity_info())
    }

    /// Handles `SYSTEM_OFF` PSCI call.
    /// Turns off the system and does not return.
    fn system_off(&self) -> ! {
        self.notify_spmd(Function::SystemOff);
        self.platform.system_off();
    }

    /// Handles `SYSTEM_OFF2` PSCI call.
    /// Suspends system to disk and never returns on success.
    fn system_off2(&self, off_type: SystemOff2Type, cookie: Cookie) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::SYSTEM_OFF2) {
            return Err(ErrorCode::NotSupported);
        }

        self.notify_spmd(Function::SystemOff2 { off_type, cookie });
        self.platform.system_off2(off_type, cookie)
    }

    /// Handles `SYSTEM_RESET` PSCI call.
    /// Resets the system and does not return.
    fn system_reset(&self) -> ! {
        self.notify_spmd(Function::SystemReset);
        self.platform.system_reset();
    }

    /// Handles `SYSTEM_RESET2` PSCI call.
    /// Initiates an architectural or vendor specific system reset. Does not return on success.
    fn system_reset2(&self, reset_type: ResetType, cookie: Cookie) -> Result<(), ErrorCode> {
        if !PsciPlatformImpl::FEATURES.contains(PsciPlatformOptionalFeatures::SYSTEM_RESET2) {
            return Err(ErrorCode::NotSupported);
        }

        self.notify_spmd(Function::SystemReset2 { reset_type, cookie });
        self.platform.system_reset2(reset_type, cookie)
    }

    /// Notify SPMD about the PSCI call.
    fn notify_spmd(&self, function: Function) {
        let mut psci_request = [0; 4];
        function.copy_to_array(&mut psci_request);

        let result = SPMD.handle_psci_event(&psci_request);
        match ReturnCode::try_from(result as i32) {
            Ok(ReturnCode::Success) => {
                // Nothing to do
            }
            Ok(ReturnCode::Error(error_code)) => {
                // The SPMD cannot prevent the PSCI state change, so we only log the error.
                log::error!("SPMD return {:?} on PSCI event {:?}", error_code, function)
            }
            Err(error) => log::error!("Failed to parse PSCI event response: {:?}", error),
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
    use arm_psci::ArchitecturalResetType;

    use super::{PsciPlatformImpl, *};
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};

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

    const ENTRY_POINT: EntryPoint = EntryPoint::Entry64 {
        entry_point_address: 0x0123_4567_89ab_cdef,
        context_id: 0xfedc_ba98_7654_3210,
    };

    const CPU1_MPIDR: Mpidr = Mpidr {
        aff0: 1,
        aff1: 0,
        aff2: 0,
        aff3: Some(0),
    };

    const INVALID_MPIDR: Mpidr = Mpidr {
        aff0: 100,
        aff1: 100,
        aff2: 100,
        aff3: Some(100),
    };

    /// The function expects the closure to power down the calling CPU. This would normally end in
    /// a function which never returns (`func() -> !`). This makes it impossible to test it so this
    /// function introduces a method for unwinding the power down call and enables further testing.
    fn expect_cpu_power_down<F>(magic: &str, f: F)
    where
        F: Fn(),
    {
        // Run closure and expect panic unwind. AssertUnwindSafe is required, because spin::Mutex
        // does not implement UnwindSafe.
        let result = catch_unwind(AssertUnwindSafe(f));

        if let Err(err) = result {
            // The closure has panicked, check for power down magic string.
            if let Some(s) = err.downcast_ref::<String>() {
                if *s == magic {
                    return;
                }
            }

            // Propagate non power down panics.
            resume_unwind(err);
        } else {
            // The closure finished without power down panic.
            panic!("Expected CPU power down did not happen");
        }
    }

    fn expect_cpu_power_down_wfi<F>(f: F)
    where
        F: Fn(),
    {
        expect_cpu_power_down(PsciPlatformImpl::POWER_DOWN_WFI_MAGIC, f);
    }

    #[test]
    fn psci_cpu_suspend() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.cpu_suspend(PowerState::StandbyOrRetention(100), ENTRY_POINT)
        );

        assert_eq!(
            Ok(()),
            psci.cpu_suspend(PowerState::StandbyOrRetention(0), ENTRY_POINT)
        );

        assert_eq!(
            Ok(()),
            psci.cpu_suspend(PowerState::StandbyOrRetention(2), ENTRY_POINT)
        );

        expect_cpu_power_down_wfi(|| {
            let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
        });

        let entry_point = psci.handle_cpu_boot();
        assert_eq!(entry_point, ENTRY_POINT);
    }

    #[test]
    fn psci_cpu_on() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.cpu_on(INVALID_MPIDR, ENTRY_POINT)
        );

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));
        assert_eq!(
            Err(ErrorCode::OnPending),
            psci.cpu_on(CPU1_MPIDR, ENTRY_POINT)
        );

        PlatformImpl::set_cpu_index(1);
        let entry_point = psci.handle_cpu_boot();
        assert_eq!(entry_point, ENTRY_POINT);

        PlatformImpl::set_cpu_index(0);
        assert_eq!(
            Err(ErrorCode::AlreadyOn),
            psci.cpu_on(CPU1_MPIDR, ENTRY_POINT)
        );
    }

    #[test]
    fn psci_cpu_off() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));

        PlatformImpl::set_cpu_index(1);
        psci.handle_cpu_boot();

        expect_cpu_power_down_wfi(|| {
            let _ = psci.cpu_off();
        });
    }

    #[test]
    fn psci_affinity_info() {
        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.affinity_info(
                INVALID_MPIDR,
                PsciCompositePowerState::CPU_POWER_LEVEL as u32
            )
        );

        assert_eq!(
            Err(ErrorCode::InvalidParameters),
            psci.affinity_info(CPU1_MPIDR, PsciPlatformImpl::MAX_POWER_LEVEL as u32)
        );

        assert_eq!(
            Ok(AffinityInfo::Off),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );

        assert_eq!(Ok(()), psci.cpu_on(CPU1_MPIDR, ENTRY_POINT));

        assert_eq!(
            Ok(AffinityInfo::OnPending),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );

        PlatformImpl::set_cpu_index(1);
        let _entry_point = psci.handle_cpu_boot();
        assert_eq!(
            Ok(AffinityInfo::On),
            psci.affinity_info(CPU1_MPIDR, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
        );
    }

    fn check_ancestor_state(psci: &Psci, cpu_index: usize, expected_states: &[PlatformPowerState]) {
        let mut cpu = psci.power_domain_tree.locked_cpu_node(cpu_index);
        assert_eq!(expected_states[0], cpu.local_state());
        psci.power_domain_tree
            .with_ancestors_locked(&mut cpu, |_cpu, ancestors| {
                for (parent, expected_state) in ancestors.iter().zip(&expected_states[1..]) {
                    assert_eq!(*expected_state, parent.local_state());
                }
            });
    }

    #[test]
    fn psci_complex_suspend_scenario() {
        // Check correct state in the power domain tree while executing the following steps.
        // * Turn on all CPUs
        // * Power down CPUs 6-13
        // * Power down CPUs 0-5
        // * Wake up CPU0
        // * Wake up CPU6

        let cpus = [
            (0, 0, 0),
            (0, 0, 1),
            (0, 0, 2),
            (0, 1, 0),
            (0, 1, 1),
            (0, 1, 2),
            (1, 0, 0),
            (1, 0, 1),
            (1, 0, 2),
            (1, 1, 0),
            (1, 1, 1),
            (1, 1, 2),
            (1, 1, 3),
        ];

        let psci = Psci::new(PsciPlatformImpl::new());

        assert_eq!(
            Ok(AffinityInfo::On),
            psci.affinity_info(
                Mpidr {
                    aff0: 0,
                    aff1: 0,
                    aff2: 0,
                    aff3: Some(0)
                },
                PsciCompositePowerState::CPU_POWER_LEVEL as u32
            )
        );

        // Turning on all secondary CPUs
        for cpu in &cpus[1..] {
            let mpidr = Mpidr {
                aff0: cpu.2,
                aff1: cpu.1,
                aff2: cpu.0,
                aff3: Some(0),
            };
            assert_eq!(Ok(()), psci.cpu_on(mpidr, ENTRY_POINT));
            assert_eq!(
                Ok(AffinityInfo::OnPending),
                psci.affinity_info(mpidr, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
            );
        }

        // Boot secondary CPUs
        for (cpu_index, cpu) in cpus.iter().enumerate().skip(1) {
            let mpidr = Mpidr {
                aff0: cpu.2,
                aff1: cpu.1,
                aff2: cpu.0,
                aff3: Some(0),
            };
            PlatformImpl::set_cpu_index(cpu_index);
            assert_eq!(ENTRY_POINT, psci.handle_cpu_boot());
            assert_eq!(
                Ok(AffinityInfo::On),
                psci.affinity_info(mpidr, PsciCompositePowerState::CPU_POWER_LEVEL as u32)
            );
        }

        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Put second cluster in power down suspend
        for (cpu_index, _cpu) in cpus.iter().enumerate().skip(6) {
            PlatformImpl::set_cpu_index(cpu_index);
            expect_cpu_power_down_wfi(|| {
                let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
            });
        }

        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Power down off all other CPUs
        for (cpu_index, _cpu) in cpus.iter().enumerate().take(6) {
            PlatformImpl::set_cpu_index(cpu_index);
            expect_cpu_power_down_wfi(|| {
                let _ = psci.cpu_suspend(PowerState::PowerDown(0), ENTRY_POINT);
            });
        }

        for cpu_index in 0..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                ],
            );
        }

        // Wake up CPU 0
        PlatformImpl::set_cpu_index(0);
        assert_eq!(ENTRY_POINT, psci.handle_cpu_boot());

        // First CPU is on
        check_ancestor_state(
            &psci,
            0,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // First cluster is on, but the CPUs are off
        for cpu_index in 1..3 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Second cluster is off
        for cpu_index in 3..6 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // The rest of the nodes are off except the top level node
        for cpu_index in 6..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Wake up CPU 6
        PlatformImpl::set_cpu_index(6);
        assert_eq!(ENTRY_POINT, psci.handle_cpu_boot());

        // CPU 0 is still on
        check_ancestor_state(
            &psci,
            0,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // First cluster is on, but the CPUs are off
        for cpu_index in 1..3 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Second cluster is off
        for cpu_index in 3..6 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // CPU 6 is now on
        check_ancestor_state(
            &psci,
            6,
            &[
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
                PlatformPowerState::RUN,
            ],
        );

        // Third cluster is on, but the CPUs are off
        for cpu_index in 7..9 {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }

        // Fourth cluster is off
        for cpu_index in 9..cpus.len() {
            check_ancestor_state(
                &psci,
                cpu_index,
                &[
                    PlatformPowerState::OFF,
                    PlatformPowerState::OFF,
                    PlatformPowerState::RUN,
                    PlatformPowerState::RUN,
                ],
            );
        }
    }

    #[test]
    fn psci_system_off() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_OFF_MAGIC, || psci.system_off());
    }

    #[test]
    fn psci_system_off2() {
        let psci = Psci::new(PsciPlatformImpl::new());

        let off_type = SystemOff2Type::HibernateOff;
        let cookie = Cookie::Cookie64(0);

        let magic = format!(
            "{} {:?} {:?}",
            PsciPlatformImpl::SYSTEM_OFF2_MAGIC,
            off_type,
            cookie
        );

        expect_cpu_power_down(magic.as_str(), || {
            let _ = psci.system_off2(off_type, cookie);
        });
    }

    #[test]
    fn psci_system_reset() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_RESET_MAGIC, || psci.system_reset());
    }

    #[test]
    fn psci_system_reset2() {
        let psci = Psci::new(PsciPlatformImpl::new());

        expect_cpu_power_down(PsciPlatformImpl::SYSTEM_RESET2_MAGIC, || {
            let _ = psci.system_reset2(
                ResetType::Architectural(ArchitecturalResetType::SystemWarmReset),
                Cookie::Cookie64(0),
            );
        });
    }
}
