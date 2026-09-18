// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Common power management related mechanisms for Arm platforms.

use core::{fmt::Debug, marker::PhantomData, ops::Range};
use rf_a_core::{
    aarch64::{dsb_ish, dsb_sy, wfi},
    reexports::{
        arm_psci::{Cookie, EntryPoint, ErrorCode, HwState, Mpidr, PowerState, ResetType},
        arm_sysregs::el0::{accessors::write_cntfrq_el0, registers::CntfrqEl0},
    },
    services::psci::{
        NodeIndexInterface, PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
        PsciPlatformInterface, PsciPlatformOptionalFeatures,
    },
};

/// Common PSCI power state across Arm platforms.
#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum ArmPowerState {
    /// Running state
    Run = 0,
    /// Standby/retention state
    Retention = 1,
    /// Off state
    Off = 2,
}

impl PlatformPowerStateInterface for ArmPowerState {
    const OFF: Self = Self::Off;
    const RUN: Self = Self::Run;

    fn power_state_type(&self) -> PowerStateType {
        match self {
            Self::Run => PowerStateType::Run,
            Self::Retention => PowerStateType::StandbyOrRetention,
            Self::Off => PowerStateType::PowerDown,
        }
    }
}

impl From<ArmPowerState> for usize {
    fn from(value: ArmPowerState) -> Self {
        value as usize
    }
}

/// Power controller interface
///
/// The role of this interface is to interact with the power controller during power management
/// calls. The GIC, timer, etc. handling is taken care of by `ArmPsciPlatform`.
pub trait PowerControllerInterface<
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const CPU_DOMAIN_COUNT: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    NodeIndex: NodeIndexInterface,
    PlatformPowerState: PlatformPowerStateInterface,
>: Send + Sync
{
    /// Power controller specific error type.
    type Error: Debug + Into<ErrorCode>;

    /// Tries to convert extended PSCI power state value into `PsciCompositePowerState`.
    fn try_parse_power_state(
        power_state: PowerState,
    ) -> Option<
        PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    >;

    /// Performs the necessary actions to turn off this cpu.
    fn power_domain_suspend(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Result<(), Self::Error>;

    /// Performs platform-specific operations after a wake-up from standby/retention states.
    fn power_domain_suspend_finish(
        &self,
        previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Result<(), Self::Error>;

    /// Allows the platform to perform platform specific validations based on hardware states.
    /// This function is optional since it is only used in OS-Initiated mode.
    fn power_domain_validate_suspend(
        &self,
        _target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Result<(), Self::Error> {
        unimplemented!("OSI mode's suspend state validation is not implemented for the platform")
    }

    /// Perform platform-specific actions to turn this cpu off.
    fn power_domain_off(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Result<(), Self::Error>;

    /// Turn on power domain, which is identified by its MPIDR.
    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), Self::Error>;

    /// Perform platform-specific actions after the CPU has been turned on.
    fn power_domain_on_finish(
        &self,
        previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    );

    /// Shuts down the system.
    fn system_off(&self) -> Result<(), Self::Error>;

    /// Resets the system, the behavior is equivalent to a hardware power-cycle sequence.
    fn system_reset(&self) -> Result<(), Self::Error>;

    /// Architectural or vendor specific reset function, optional.
    fn system_reset2(&self, _reset_type: ResetType, _cookie: Cookie) -> Result<(), Self::Error> {
        unimplemented!("SYSTEM_RESET2 is not implemented for the platform")
    }

    /// Returns the power state for `CPU_DEFAULT_SUSPEND`, optional.
    fn cpu_default_suspend_power_state(&self) -> PowerState {
        unimplemented!("CPU_DEFAULT_SUSPEND is not implemented for the platform")
    }

    /// Returns the true hardware state of a power domain, optional.
    fn node_hw_state(&self, _mpidr: Mpidr, _power_level: u32) -> Result<HwState, Self::Error> {
        unimplemented!("NODE_HW_STATE is not implemented for the platform")
    }

    /// Returns the power state for `SYSTEM_SUSPEND`, optional.
    fn sys_suspend_power_state(
        &self,
    ) -> PsciCompositePowerState<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        NodeIndex,
        PlatformPowerState,
    > {
        unimplemented!("SYSTEM_SUSPEND is not implemented for the platform")
    }

    /// Returns true if an optional feature is supported by the platform.
    fn is_feature_supported(&self, feature: PsciPlatformOptionalFeatures) -> bool;
}

/// Provides an interface for handling the GIC state during power management.
pub trait GicInterface: Send + Sync {
    /// Enables the GIC CPU interface of the calling CPU.
    fn cpu_interface_enable(&self);
    /// Disables the GIC CPU interface of the calling CPU.
    fn cpu_interface_disable(&self);

    /// Initializes the CPU related GIC components (i.e. CPU interface, redistributor).
    fn cpu_init(&self);
    /// Deinitializes the CPU related GIC components (i.e. CPU interface, redistributor).
    fn cpu_deinit(&self);

    /// Saves the GIC state before system suspend.
    fn system_save(&self);
    /// Restores the GIC state after system suspend.
    fn system_restore(&self);
}

/// Provides an interface for handling the timer state during power management.
pub trait TimerInterface: Send + Sync {
    /// Initializes the timer.
    fn init(&self);
    /// Returns the timer frequency in Hz.
    fn frequency(&self) -> u32;
}

/// Creates topology array for systems where all the clusters have the same number of CPUs.
#[macro_export]
macro_rules! topology {
    ($cluster_count:expr, $cpu_per_cluster:expr) => {{
        const TOPOLOGY: [usize; 2 + $cluster_count] = {
            let mut topology = [0; 2 + $cluster_count];

            topology[0] = 1;
            topology[1] = $cluster_count;

            let mut i = 0;
            loop {
                if i >= $cluster_count {
                    break;
                }
                topology[i + 2] = $cpu_per_cluster;
                i += 1;
            }
            topology
        };

        &TOPOLOGY
    }};
}

/// Common PSCI platform implementation across Arm platforms.
///
/// This struct further delegates a smaller set of platform specific hooks via traits:
/// * `PowerControllerInterface`
/// * `GicInterface`
/// * `TimerInterface`
pub struct ArmPsciPlatform<
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const CPU_DOMAIN_COUNT: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    NodeIndex: NodeIndexInterface,
    PlatformPowerState: PlatformPowerStateInterface,
    PowerController: PowerControllerInterface<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    Gic: GicInterface,
    Timer: TimerInterface,
> {
    topology: &'static [usize],
    power_controller: PowerController,
    gic: Gic,
    timer: Timer,
    valid_ns_ranges: &'static [Range<usize>],
    _phantom: PhantomData<(NodeIndex, PlatformPowerState)>,
}

impl<
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const CPU_DOMAIN_COUNT: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    NodeIndex: NodeIndexInterface,
    PlatformPowerState: PlatformPowerStateInterface,
    PowerController: PowerControllerInterface<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    Gic: GicInterface,
    Timer: TimerInterface,
>
    ArmPsciPlatform<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        NodeIndex,
        PlatformPowerState,
        PowerController,
        Gic,
        Timer,
    >
{
    /// Creates a new instance
    pub fn new(
        topology: &'static [usize],
        power_controller: PowerController,
        gic: Gic,
        timer: Timer,
        valid_ns_ranges: &'static [Range<usize>],
    ) -> Self {
        timer.init();
        write_cntfrq_el0(CntfrqEl0::from_bits_retain(timer.frequency().into()));

        Self {
            topology,
            power_controller,
            gic,
            timer,
            valid_ns_ranges,
            _phantom: PhantomData,
        }
    }

    /// Performs WFI to execute power down.
    fn power_down() -> ! {
        const CPU_OFF_WFI_RETRY_COUNT: usize = 32;

        dsb_sy();

        // Execute a wfi which, in most cases, will allow the power controller to physically power
        // down this cpu. Under some circumstances that may be denied. Hopefully this is transient,
        // retrying a few times should power down.
        for _ in 0..CPU_OFF_WFI_RETRY_COUNT {
            wfi();
        }

        // Wake up wasn't transient. System is probably in a bad state.
        panic!("Could not power off CPU/system");
    }
}

impl<
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const CPU_DOMAIN_COUNT: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    NodeIndex: NodeIndexInterface,
    PlatformPowerState: PlatformPowerStateInterface,
    PowerController: PowerControllerInterface<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    Gic: GicInterface,
    Timer: TimerInterface,
> PsciPlatformInterface<STATE_COUNT, MAX_POWER_LEVEL, CPU_DOMAIN_COUNT, NON_CPU_DOMAIN_COUNT>
    for ArmPsciPlatform<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        NodeIndex,
        PlatformPowerState,
        PowerController,
        Gic,
        Timer,
    >
{
    const POWER_DOMAIN_COUNT: usize = CPU_DOMAIN_COUNT + NON_CPU_DOMAIN_COUNT;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::empty();

    type PlatformPowerState = PlatformPowerState;

    type NodeIndex = NodeIndex;

    fn topology(&self) -> &'static [usize] {
        self.topology
    }

    fn try_parse_power_state(
        power_state: PowerState,
    ) -> Option<
        PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    > {
        PowerController::try_parse_power_state(power_state)
    }

    fn cpu_standby(&self, cpu_state: Self::PlatformPowerState) {
        assert!(cpu_state.power_state_type() == PowerStateType::StandbyOrRetention);

        // Enter standby state. DSB is good practice before using WFI to enter low power states.
        dsb_ish();
        wfi();
    }

    fn power_domain_suspend(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) {
        // Retention only at cpu level. Just return as nothing is to be done for retention.
        if target_state.cpu_level_state().power_state_type() == PowerStateType::StandbyOrRetention {
            return;
        }

        // Make sure that at least the CPU is powered down.
        assert_eq!(
            target_state.cpu_level_state().power_state_type(),
            PowerStateType::PowerDown
        );

        // Prevent interrupts from spuriously waking up this cpu.
        self.gic.cpu_interface_disable();

        // Perform the common system power down operations.
        if target_state.highest_level_state().power_state_type() == PowerStateType::PowerDown {
            self.gic.system_save();
        }

        self.power_controller
            .power_domain_suspend(target_state)
            .unwrap();
    }

    fn power_domain_suspend_finish(
        &self,
        previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) {
        // Nothing to be done on waking up from retention at CPU level.
        if previous_state.cpu_level_state().power_state_type() == PowerStateType::StandbyOrRetention
        {
            return;
        }

        // Make sure the CPU was in power down state.
        assert_eq!(
            previous_state.cpu_level_state().power_state_type(),
            PowerStateType::PowerDown
        );

        // Clean up power down state in the power controller.
        self.power_controller
            .power_domain_suspend_finish(previous_state)
            .unwrap();

        // Perform the common system power on operations.
        if previous_state.highest_level_state().power_state_type() == PowerStateType::PowerDown {
            self.gic.system_restore();
            self.timer.init();
        }

        write_cntfrq_el0(CntfrqEl0::from_bits_retain(self.timer.frequency().into()));

        self.gic.cpu_interface_enable();
    }

    fn power_domain_validate_suspend(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) -> Result<(), ErrorCode> {
        self.power_controller
            .power_domain_validate_suspend(target_state)
            .map_err(|err| err.into())
    }

    fn power_domain_off(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) {
        // Make sure that at least the CPU is powered down.
        assert_eq!(
            target_state.cpu_level_state().power_state_type(),
            PowerStateType::PowerDown
        );

        self.gic.cpu_deinit();
        self.power_controller
            .power_domain_off(target_state)
            .unwrap();
    }

    fn power_domain_power_down(
        &self,
        _target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) {
    }

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode> {
        self.power_controller
            .power_domain_on(mpidr)
            .map_err(|err| err.into())
    }

    fn power_domain_on_finish(
        &self,
        previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            Self::PlatformPowerState,
        >,
    ) {
        // Make sure the CPU was turned off.
        assert_eq!(
            previous_state.cpu_level_state().power_state_type(),
            PowerStateType::PowerDown
        );

        self.power_controller.power_domain_on_finish(previous_state);

        // Perform the common system power on operations.
        if previous_state.highest_level_state().power_state_type() == PowerStateType::PowerDown {
            self.gic.system_restore();
            self.timer.init();
        }

        write_cntfrq_el0(CntfrqEl0::from_bits_retain(self.timer.frequency().into()));

        self.gic.cpu_init();
    }

    fn system_off(&self) -> ! {
        self.power_controller.system_off().unwrap();
        Self::power_down();
    }

    fn system_reset(&self) -> ! {
        self.power_controller.system_reset().unwrap();
        Self::power_down();
    }

    fn system_reset2(&self, reset_type: ResetType, cookie: Cookie) -> Result<(), ErrorCode> {
        self.power_controller
            .system_reset2(reset_type, cookie)
            .map_err(|err| err.into())?;
        Self::power_down();
    }

    fn cpu_default_suspend_power_state(&self) -> PowerState {
        self.power_controller.cpu_default_suspend_power_state()
    }

    fn node_hw_state(&self, mpidr: Mpidr, power_level: u32) -> Result<HwState, ErrorCode> {
        self.power_controller
            .node_hw_state(mpidr, power_level)
            .map_err(|err| err.into())
    }

    fn sys_suspend_power_state(
        &self,
    ) -> PsciCompositePowerState<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        Self::NodeIndex,
        Self::PlatformPowerState,
    > {
        self.power_controller.sys_suspend_power_state()
    }

    fn is_valid_ns_entrypoint(&self, entry: &EntryPoint) -> bool {
        let entry_point_address = entry.entry_point_address() as usize;

        self.valid_ns_ranges.is_empty()
            || self
                .valid_ns_ranges
                .iter()
                .any(|range| range.contains(&entry_point_address))
    }

    fn is_feature_supported(&self, feature: PsciPlatformOptionalFeatures) -> bool {
        self.power_controller.is_feature_supported(feature)
    }
}
