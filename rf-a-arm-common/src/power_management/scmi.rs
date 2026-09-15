// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! SCMI (SCP) based power management.

pub mod mhu_scmi_doorbell;
mod power_state;

use crate::power_management::{
    ArmPowerState, PowerControllerInterface,
    scmi::power_state::{ScmiCompositePowerState, ScmiPowerState},
};
use arm_scmi::{
    Error, ScmiAgent,
    protocol::{
        power_domain::PowerDomainCommandMessageId,
        system_power::{
            StandardSystemState, SystemPowerCommandMessageId, SystemPowerStateSetAttributes,
            SystemPowerStateSetFlags, SystemState,
        },
    },
    transport::Transport,
};
use core::marker::PhantomData;
use rf_a_core::{
    context::CoreIndex,
    reexports::{
        arm_psci::{self, Cookie, HwState, Mpidr, PowerState, ResetType},
        arm_sysregs::el1::registers::MpidrEl1,
        spin::mutex::SpinMutex,
    },
    services::psci::{
        NodeIndexInterface, PlatformPowerStateInterface, PsciCompositePowerState,
        PsciPlatformOptionalFeatures,
    },
};

/// The trait defines an interface for converting between CPU index and SCMI domain ID values.
pub trait DomainMap: Send + Sync {
    /// Converts CPU index to domain ID.
    fn cpu_index_to_domain_id(cpu_index: usize) -> u16;
}

/// `DomainMap` implementation where the CPU indices are the same as the SCMI domain IDs.
pub struct IdentityDomainMap;

impl DomainMap for IdentityDomainMap {
    fn cpu_index_to_domain_id(cpu_index: usize) -> u16 {
        cpu_index.try_into().unwrap()
    }
}

/// `ScmiPowerController` error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScmiPowerControllerError(arm_scmi::Error);

impl From<arm_scmi::Error> for ScmiPowerControllerError {
    fn from(value: arm_scmi::Error) -> Self {
        Self(value)
    }
}

impl From<ScmiPowerControllerError> for arm_psci::ErrorCode {
    fn from(value: ScmiPowerControllerError) -> Self {
        match value.0 {
            Error::InvalidPowerState(_) | Error::InvalidSystemState(_) => {
                arm_psci::ErrorCode::InvalidParameters
            }
            _ => arm_psci::ErrorCode::InternalFailure,
        }
    }
}

/// SCMI based `PowerControllerInterface` implementation.
pub struct ScmiPowerController<CoreIndexImpl: CoreIndex, T: Transport, DM: DomainMap> {
    scmi_agent: SpinMutex<ScmiAgent<T>>,
    features: PsciPlatformOptionalFeatures,
    _phantom: PhantomData<(CoreIndexImpl, DM)>,
}

impl<CoreIndexImpl: CoreIndex, T: Transport, DM: DomainMap>
    ScmiPowerController<CoreIndexImpl, T, DM>
{
    /// Creates a new instance and detects the available features of the platform, based on the
    /// reported SCMI features.
    pub fn new(transport: T) -> Result<Self, Error> {
        let mut scmi_agent = ScmiAgent::new(transport)?;

        let mut power_domain_management = scmi_agent.power_domain_management()?;

        // At least POWER_STATE_SET must be supported
        power_domain_management
            .protocol_message_attributes(PowerDomainCommandMessageId::StateSet)?;

        let mut features = PsciPlatformOptionalFeatures::empty();

        if power_domain_management
            .protocol_message_attributes(PowerDomainCommandMessageId::StateGet)
            .is_ok()
        {
            features |= PsciPlatformOptionalFeatures::NODE_HW_STATE;
        }

        let mut system_power_management = scmi_agent.system_power_management()?;

        if let Ok(bits) = system_power_management
            .protocol_message_attributes(SystemPowerCommandMessageId::StateSet)
        {
            let attributes = SystemPowerStateSetAttributes::from_bits_retain(bits);

            if attributes.contains(SystemPowerStateSetAttributes::SYSTEM_SUSPEND_SUPPORT) {
                features |= PsciPlatformOptionalFeatures::SYSTEM_SUSPEND;
            }

            if attributes.contains(SystemPowerStateSetAttributes::SYSTEM_WARM_RESET_SUPPORT) {
                features |= PsciPlatformOptionalFeatures::SYSTEM_RESET2;
            }
        }

        Ok(Self {
            scmi_agent: SpinMutex::new(scmi_agent),
            features,
            _phantom: PhantomData,
        })
    }

    /// Returns the local CPU's SCMI domain ID.
    fn local_domain_id(&self) -> u16 {
        DM::cpu_index_to_domain_id(CoreIndexImpl::local_index())
    }

    /// Sets the system power state.
    fn system_power_state_set(
        &self,
        system_state: SystemState,
        flags: SystemPowerStateSetFlags,
    ) -> Result<(), ScmiPowerControllerError> {
        Ok(self
            .scmi_agent
            .lock()
            .system_power_management()?
            .system_power_state_set(system_state, flags)?)
    }
}

impl<
    const STATE_COUNT: usize,
    const MAX_POWER_LEVEL: usize,
    const CPU_DOMAIN_COUNT: usize,
    const NON_CPU_DOMAIN_COUNT: usize,
    NodeIndex: NodeIndexInterface,
    CoreIndexImpl: CoreIndex,
    T: Transport + Send,
    DM: DomainMap,
>
    PowerControllerInterface<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        NodeIndex,
        ArmPowerState,
    > for ScmiPowerController<CoreIndexImpl, T, DM>
{
    type Error = ScmiPowerControllerError;

    fn try_parse_power_state(
        power_state: PowerState,
    ) -> Option<
        PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            ArmPowerState,
        >,
    > {
        const POWER_LEVEL_STATE_MASK: u32 = 0x0000_0fff;
        const ARM_LOCAL_PSTATE_WIDTH: u32 = 4;
        const ARM_LOCAL_PSTATE_MASK: u32 = (1 << ARM_LOCAL_PSTATE_WIDTH) - 1;
        // last_at_power_level is encoded in the bits immediately following the state ID bits
        // for each power level.
        let last_at_pwr_lvl_shift: u32 = ARM_LOCAL_PSTATE_WIDTH * (MAX_POWER_LEVEL as u32 + 1);
        const CPU_RETENTION: u32 = 0x001;
        const CPU_POWER_DOWN: u32 = 0x002;
        const CLUSTER_POWER_DOWN: u32 = 0x022;
        const SYSTEM_POWER_DOWN: u32 = 0x222;

        if let PowerState::StandbyOrRetention(CPU_RETENTION) = power_state {
            let mut states = [ArmPowerState::Run; STATE_COUNT];
            states[0] = ArmPowerState::Retention;
            return Some(PsciCompositePowerState::new(states));
        }

        let value = match power_state {
            PowerState::PowerDown(v) => v,
            _ => return None,
        };

        let mut states = [ArmPowerState::Run; STATE_COUNT];

        match value & POWER_LEVEL_STATE_MASK {
            CPU_POWER_DOWN => {
                states[0] = ArmPowerState::Off;
            }
            CLUSTER_POWER_DOWN => {
                states[0] = ArmPowerState::Off;
                states[1] = ArmPowerState::Off;
            }
            // Ensure that the system power domain level is never suspended via PSCI
            // CPU_SUSPEND API. System suspend is only supported via PSCI SYSTEM_SUSPEND
            // API.
            SYSTEM_POWER_DOWN => {
                states[0] = ArmPowerState::Off;
                states[1] = ArmPowerState::Off;
            }
            _ => return None,
        };

        let last_at_power_level =
            ((value >> last_at_pwr_lvl_shift) & ARM_LOCAL_PSTATE_MASK) as usize;

        if last_at_power_level > MAX_POWER_LEVEL {
            return None;
        }

        Some(PsciCompositePowerState::new_with_last_power_level(
            states,
            last_at_power_level,
        ))
    }

    fn power_domain_suspend(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            ArmPowerState,
        >,
    ) -> Result<(), Self::Error> {
        assert_ne!(target_state.cpu_level_state(), ArmPowerState::RUN);

        if target_state.highest_level_state() == ArmPowerState::OFF {
            self.system_power_state_set(
                SystemState::Standard(StandardSystemState::SystemSuspend),
                SystemPowerStateSetFlags::empty(),
            )?;
            return Ok(());
        }

        let scmi_composite_power_state =
            ScmiCompositePowerState::create_suspend_state(target_state);

        self.scmi_agent
            .lock()
            .power_domain_management()?
            .power_state_set(
                self.local_domain_id(),
                scmi_composite_power_state.into(),
                true,
            )?;

        Ok(())
    }

    fn power_domain_suspend_finish(
        &self,
        _previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            ArmPowerState,
        >,
    ) -> Result<(), Self::Error> {
        // Nothing to do.
        Ok(())
    }

    fn power_domain_off(
        &self,
        target_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            ArmPowerState,
        >,
    ) -> Result<(), Self::Error> {
        let scmi_composite_power_state = ScmiCompositePowerState::create_off_state(target_state);

        self.scmi_agent
            .lock()
            .power_domain_management()?
            .power_state_set(
                self.local_domain_id(),
                scmi_composite_power_state.into(),
                true,
            )?;

        Ok(())
    }

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), Self::Error> {
        let scmi_composite_power_state =
            ScmiCompositePowerState::create_on_state::<STATE_COUNT, MAX_POWER_LEVEL>();

        let core_index = CoreIndexImpl::mpidr_to_index(MpidrEl1::from_psci_mpidr(u64::from(mpidr)));
        let domain_id = DM::cpu_index_to_domain_id(core_index);

        self.scmi_agent
            .lock()
            .power_domain_management()?
            .power_state_set(domain_id, scmi_composite_power_state.into(), true)?;

        Ok(())
    }

    fn power_domain_on_finish(
        &self,
        _previous_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            ArmPowerState,
        >,
    ) {
        // Nothing to do.
    }

    fn system_off(&self) -> Result<(), Self::Error> {
        self.system_power_state_set(
            SystemState::Standard(StandardSystemState::SystemShutdown),
            SystemPowerStateSetFlags::GRACEFUL,
        )
    }

    fn system_reset(&self) -> Result<(), Self::Error> {
        self.system_power_state_set(
            SystemState::Standard(StandardSystemState::SystemColdReset),
            SystemPowerStateSetFlags::GRACEFUL,
        )
    }

    fn system_reset2(&self, reset_type: ResetType, _cookie: Cookie) -> Result<(), Self::Error> {
        if reset_type != ResetType::Architectural(arm_psci::ArchitecturalResetType::SystemWarmReset)
        {
            return Err(ScmiPowerControllerError(
                arm_scmi::Error::InvalidSystemState(0),
            ));
        }

        self.system_power_state_set(
            SystemState::Standard(StandardSystemState::SystemWarmReset),
            SystemPowerStateSetFlags::GRACEFUL,
        )
    }

    fn node_hw_state(&self, mpidr: Mpidr, power_level: u32) -> Result<HwState, Self::Error> {
        let core_index = CoreIndexImpl::mpidr_to_index(MpidrEl1::from_psci_mpidr(u64::from(mpidr)));
        let domain_id = DM::cpu_index_to_domain_id(core_index);

        let state = self
            .scmi_agent
            .lock()
            .power_domain_management()?
            .power_state_get(domain_id)?;

        let scmi_composite_power_state = ScmiCompositePowerState::try_from(state)?;

        Ok(scmi_composite_power_state
            .state(power_level as usize)
            .unwrap_or(ScmiPowerState::On)
            .into())
    }

    fn sys_suspend_power_state(
        &self,
    ) -> PsciCompositePowerState<
        STATE_COUNT,
        MAX_POWER_LEVEL,
        CPU_DOMAIN_COUNT,
        NON_CPU_DOMAIN_COUNT,
        NodeIndex,
        ArmPowerState,
    > {
        PsciCompositePowerState::OFF
    }

    fn is_feature_supported(&self, feature: PsciPlatformOptionalFeatures) -> bool {
        self.features.contains(feature)
    }
}
