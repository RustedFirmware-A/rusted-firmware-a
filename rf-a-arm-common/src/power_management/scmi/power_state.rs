// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Common SCMI power types for Arm platforms.

use arm_scmi::protocol::power_domain::{PowerState as ArmScmiPowerState, PowerStateType};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use rf_a_core::{
    reexports::arm_psci::HwState,
    services::psci::{NodeIndexInterface, PlatformPowerStateInterface, PsciCompositePowerState},
};

/// Power state of a level as represented in the SCMI power state related commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ScmiPowerState {
    Off = 0,
    On = 1,
    Sleep = 2,
}

impl From<ScmiPowerState> for HwState {
    fn from(value: ScmiPowerState) -> Self {
        match value {
            ScmiPowerState::Off => HwState::Off,
            ScmiPowerState::On => HwState::On,
            ScmiPowerState::Sleep => HwState::Standby,
        }
    }
}

/// Power state of all levels as represented in the SCMI power state related commands.
/// `states` contains the states of the individual levels and `max_level` encodes the highest level
/// that has a valid power state encoded in the `states`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScmiCompositePowerState {
    states: [ScmiPowerState; Self::STATE_COUNT],
    max_level: u8,
}

impl ScmiCompositePowerState {
    const STATE_COUNT: usize = 4;
    const STATE_MASK: u32 = 0b1111;
    const STATE_SHIFT: usize = 4;
    const MAX_LEVEL_MASK: u32 = 0b1111;
    const MAX_LEVEL_SHIFT: u32 = 16;

    /// Returns state if the level is valid.
    pub fn state(&self, level: usize) -> Option<ScmiPowerState> {
        if level <= self.max_level() {
            Some(self.states[level])
        } else {
            None
        }
    }

    /// Returns max valid level.
    pub fn max_level(&self) -> usize {
        self.max_level.into()
    }

    /// Creates a new instance from `PsciCompositePowerState` where the CPU level state is set to
    /// Sleep and the rest of the states to Off. `max_level` is set to the highest level that is
    /// turned off.
    pub fn create_suspend_state<
        const STATE_COUNT: usize,
        const MAX_POWER_LEVEL: usize,
        const CPU_DOMAIN_COUNT: usize,
        const NON_CPU_DOMAIN_COUNT: usize,
        NodeIndex: NodeIndexInterface,
        PlatformPowerState: PlatformPowerStateInterface,
    >(
        psci_composite_power_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Self {
        const {
            assert!(STATE_COUNT <= ScmiCompositePowerState::STATE_COUNT);
        }

        Self {
            states: [
                ScmiPowerState::Sleep,
                ScmiPowerState::Off,
                ScmiPowerState::Off,
                ScmiPowerState::Off,
            ],
            max_level: psci_composite_power_state
                .find_highest_non_run_level()
                .unwrap_or(MAX_POWER_LEVEL) as u8,
        }
    }

    /// Creates a new instance from `PsciCompositePowerState` where levels are set to `Off` and
    /// `max_level` is set to the highest level that is turned off.
    pub fn create_off_state<
        const STATE_COUNT: usize,
        const MAX_POWER_LEVEL: usize,
        const CPU_DOMAIN_COUNT: usize,
        const NON_CPU_DOMAIN_COUNT: usize,
        NodeIndex: NodeIndexInterface,
        PlatformPowerState: PlatformPowerStateInterface,
    >(
        psci_composite_power_state: &PsciCompositePowerState<
            STATE_COUNT,
            MAX_POWER_LEVEL,
            CPU_DOMAIN_COUNT,
            NON_CPU_DOMAIN_COUNT,
            NodeIndex,
            PlatformPowerState,
        >,
    ) -> Self {
        const {
            assert!(STATE_COUNT <= ScmiCompositePowerState::STATE_COUNT);
        }

        // At least the CPU level should be specified to be OFF.
        assert_eq!(
            psci_composite_power_state.cpu_level_state(),
            PlatformPowerState::OFF
        );

        // Cannot turn off system level power domain.
        assert_eq!(
            psci_composite_power_state.highest_level_state(),
            PlatformPowerState::RUN
        );

        Self {
            states: [ScmiPowerState::Off; Self::STATE_COUNT],
            max_level: psci_composite_power_state
                .find_highest_non_run_level()
                .unwrap_or(MAX_POWER_LEVEL) as u8,
        }
    }

    /// Creates a new instance where levels are set to `On` up to `MAX_POWER_LEVEL`.
    pub fn create_on_state<const STATE_COUNT: usize, const MAX_POWER_LEVEL: usize>() -> Self {
        const {
            assert!(MAX_POWER_LEVEL + 1 == STATE_COUNT);
            assert!(STATE_COUNT <= ScmiCompositePowerState::STATE_COUNT);
        }

        let mut states = [ScmiPowerState::Off; ScmiCompositePowerState::STATE_COUNT];
        states[..STATE_COUNT].fill(ScmiPowerState::On);

        Self {
            states,
            max_level: MAX_POWER_LEVEL as u8,
        }
    }
}

impl From<ScmiCompositePowerState> for ArmScmiPowerState {
    fn from(value: ScmiCompositePowerState) -> Self {
        // 31  20 19       16 15      12 11       8 7        4 3         0
        // +-------------------------------------------------------------+
        // | SBZ | Max level |  Level 3 |  Level 2 |  Level 1 |  Level 0 |
        // |     |           |   state  |   state  |   state  |   state  |
        // +-------------------------------------------------------------+

        let mut state_id = 0;

        for (level, state) in value
            .states
            .iter()
            .enumerate()
            .take(ScmiCompositePowerState::STATE_COUNT)
        {
            state_id |= (u32::from(u8::from(*state)) & ScmiCompositePowerState::STATE_MASK)
                << (level * ScmiCompositePowerState::STATE_SHIFT);
        }
        state_id |= (u32::from(value.max_level) & ScmiCompositePowerState::MAX_LEVEL_MASK)
            << ScmiCompositePowerState::MAX_LEVEL_SHIFT;

        Self::new(PowerStateType::ContextPreserved, state_id)
    }
}

impl TryFrom<ArmScmiPowerState> for ScmiCompositePowerState {
    type Error = arm_scmi::Error;

    fn try_from(value: ArmScmiPowerState) -> Result<Self, Self::Error> {
        let mut instance = Self {
            states: [ScmiPowerState::Off; 4],
            max_level: 0,
        };

        let state_id = value.state_id();
        for (level, state) in instance
            .states
            .iter_mut()
            .enumerate()
            .take(ScmiCompositePowerState::STATE_COUNT)
        {
            let level_state = ((state_id >> (level * Self::STATE_SHIFT)) & Self::STATE_MASK) as u8;

            *state = ScmiPowerState::try_from(level_state)
                .map_err(|_| arm_scmi::Error::InvalidPowerState(state_id))?;
        }

        instance.max_level = ((state_id >> Self::MAX_LEVEL_SHIFT) & Self::MAX_LEVEL_MASK) as u8;
        if usize::from(instance.max_level) >= Self::STATE_COUNT {
            return Err(arm_scmi::Error::InvalidPowerState(state_id));
        }

        Ok(instance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power_management::ArmPowerState;

    type TestPsciCompositePowerState = PsciCompositePowerState<4, 3, 2, 1, u8, ArmPowerState>;

    #[test]
    fn create_suspend_state_level0() {
        let psci_composite_power_state: TestPsciCompositePowerState =
            TestPsciCompositePowerState::new([
                ArmPowerState::Off,
                ArmPowerState::Run,
                ArmPowerState::Run,
                ArmPowerState::Run,
            ]);

        assert_eq!(
            ScmiCompositePowerState::create_suspend_state(&psci_composite_power_state),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::Sleep,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off
                ],
                max_level: 0
            }
        )
    }

    #[test]
    fn create_suspend_state_level2() {
        let psci_composite_power_state: TestPsciCompositePowerState =
            TestPsciCompositePowerState::new([
                ArmPowerState::Off,
                ArmPowerState::Off,
                ArmPowerState::Off,
                ArmPowerState::Run,
            ]);

        assert_eq!(
            ScmiCompositePowerState::create_suspend_state(&psci_composite_power_state),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::Sleep,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off
                ],
                max_level: 2
            }
        )
    }

    #[test]
    fn create_off_state_level0() {
        let psci_composite_power_state: TestPsciCompositePowerState =
            TestPsciCompositePowerState::new([
                ArmPowerState::Off,
                ArmPowerState::Run,
                ArmPowerState::Run,
                ArmPowerState::Run,
            ]);

        assert_eq!(
            ScmiCompositePowerState::create_off_state(&psci_composite_power_state),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off
                ],
                max_level: 0
            }
        )
    }

    #[test]
    fn create_off_state_level2() {
        let psci_composite_power_state: TestPsciCompositePowerState =
            TestPsciCompositePowerState::new([
                ArmPowerState::Off,
                ArmPowerState::Off,
                ArmPowerState::Off,
                ArmPowerState::Run,
            ]);

        assert_eq!(
            ScmiCompositePowerState::create_off_state(&psci_composite_power_state),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off
                ],
                max_level: 2
            }
        )
    }

    #[test]
    fn create_on_state_level0() {
        assert_eq!(
            ScmiCompositePowerState::create_on_state::<1, 0>(),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::On,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off,
                    ScmiPowerState::Off
                ],
                max_level: 0
            }
        )
    }

    #[test]
    fn create_on_state_level2() {
        assert_eq!(
            ScmiCompositePowerState::create_on_state::<3, 2>(),
            ScmiCompositePowerState {
                states: [
                    ScmiPowerState::On,
                    ScmiPowerState::On,
                    ScmiPowerState::On,
                    ScmiPowerState::Off
                ],
                max_level: 2
            }
        )
    }

    #[test]
    fn into_scmi_power_state() {
        let composite_state = ScmiCompositePowerState {
            states: [
                ScmiPowerState::Sleep,
                ScmiPowerState::Off,
                ScmiPowerState::Off,
                ScmiPowerState::Off,
            ],
            max_level: 0,
        };

        assert_eq!(
            ArmScmiPowerState::from(composite_state),
            ArmScmiPowerState::new(PowerStateType::ContextPreserved, 0x0_0002)
        );

        let composite_state = ScmiCompositePowerState {
            states: [
                ScmiPowerState::Sleep,
                ScmiPowerState::On,
                ScmiPowerState::On,
                ScmiPowerState::Off,
            ],
            max_level: 2,
        };

        assert_eq!(
            ArmScmiPowerState::from(composite_state),
            ArmScmiPowerState::new(PowerStateType::ContextPreserved, 0x2_0112)
        );
    }
}
