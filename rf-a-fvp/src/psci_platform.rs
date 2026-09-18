// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{Fvp, FvpCoreIndex, GIC, config::FVP_CLUSTER_COUNT};
use arm_fvp_base_pac::{
    power_controller::{FvpPowerController, SystemStatus},
    system::{FvpSystemPeripheral, SystemConfigFunction},
};
use rf_a_arm_common::power_management::{
    ArmPowerState, ArmPsciPlatform, GicInterface, PowerControllerInterface,
    generic_timer::GenericMmioTimer,
};
use rf_a_core::{
    context::CoreIndex,
    platform::Platform,
    reexports::{
        arm_gic::gicv3::{GicDistributorContext, GicRedistributorContext},
        arm_psci::{ErrorCode, HwState, Mpidr, PowerState},
        arm_sysregs::el1::accessors::read_mpidr_el1,
        spin::mutex::SpinMutex,
    },
    services::psci::{CPU_POWER_LEVEL, PsciCompositePowerState, PsciPlatformOptionalFeatures},
};

/// GIC context to save on different levels of power down.
struct FvpGicContext {
    distributor_context: GicDistributorContext<
        { GicDistributorContext::spi_count(988) },
        { GicDistributorContext::espi_count(1024) },
    >,
    redistributor_context: GicRedistributorContext<{ GicRedistributorContext::ppi_count(96) }>,
}

impl FvpGicContext {
    /// Creates new instance.
    const fn new() -> Self {
        Self {
            distributor_context: GicDistributorContext::new(),
            redistributor_context: GicRedistributorContext::new(),
        }
    }
}

static GIC_CONTEXT: SpinMutex<FvpGicContext> = SpinMutex::new(FvpGicContext::new());

/// `GicInterface` for the FVP platform.
pub struct FvpGic;

impl GicInterface for FvpGic {
    fn cpu_interface_enable(&self) {
        GIC.get().unwrap().cpu_interface_enable();
    }

    fn cpu_interface_disable(&self) {
        GIC.get().unwrap().cpu_interface_disable();
    }

    fn cpu_init(&self) {
        let gic = GIC.get().unwrap();
        gic.redistributor_init(&Fvp::GIC_CONFIG);
        gic.cpu_interface_enable();
    }

    fn cpu_deinit(&self) {
        let gic = GIC.get().unwrap();
        gic.cpu_interface_disable();
        gic.redistributor_off();
    }

    fn system_save(&self) {
        let mut context = GIC_CONTEXT.lock();
        let gic = GIC.get().unwrap();

        gic.redistributor_save(&mut context.redistributor_context);
        gic.distributor_save(&mut context.distributor_context);
    }

    fn system_restore(&self) {
        let context = GIC_CONTEXT.lock();
        let gic = GIC.get().unwrap();

        gic.distributor_restore(&context.distributor_context);
        gic.redistributor_restore(&context.redistributor_context);
    }
}

const _: () = assert!(
    (FVP_CLUSTER_COUNT > 0) && (FVP_CLUSTER_COUNT <= 256),
    "Invalid FVP cluster count"
);

const PSCI_MAX_POWER_LEVEL: usize = 2;
pub const PSCI_STATE_COUNT: usize = PSCI_MAX_POWER_LEVEL + 1;
const PSCI_NON_CPU_DOMAIN_COUNT: usize = 1 + FVP_CLUSTER_COUNT;

type FvpCompositePowerState = PsciCompositePowerState<
    PSCI_STATE_COUNT,
    PSCI_MAX_POWER_LEVEL,
    { FvpCoreIndex::CORE_COUNT },
    PSCI_NON_CPU_DOMAIN_COUNT,
    u8,
    ArmPowerState,
>;

pub type FvpPsciPlatformImpl<'a> = ArmPsciPlatform<
    PSCI_STATE_COUNT,
    PSCI_MAX_POWER_LEVEL,
    { FvpCoreIndex::CORE_COUNT },
    PSCI_NON_CPU_DOMAIN_COUNT,
    u8,
    ArmPowerState,
    FvpPsciPowerController<'a>,
    FvpGic,
    GenericMmioTimer<'a>,
>;

/// `FvpPsciPowerController` error type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FvpPsciPowerControllerError(ErrorCode);

impl From<FvpPsciPowerControllerError> for ErrorCode {
    fn from(value: FvpPsciPowerControllerError) -> Self {
        value.0
    }
}

/// `PowerControllerInterface` implementation for FVP that uses the power controller for CPU and
/// cluster level power management and the system peripheral for system level power management.
pub struct FvpPsciPowerController<'a> {
    power_controller: SpinMutex<FvpPowerController<'a>>,
    system: SpinMutex<FvpSystemPeripheral<'a>>,
}

impl<'a> FvpPsciPowerController<'a> {
    const CLUSTER_POWER_LEVEL: usize = 1;

    pub fn new(power_controller: FvpPowerController<'a>, system: FvpSystemPeripheral<'a>) -> Self {
        Self {
            power_controller: SpinMutex::new(power_controller),
            system: SpinMutex::new(system),
        }
    }

    fn power_domain_on_finish_common(&self, previous_state: &FvpCompositePowerState) {
        let mpidr = read_mpidr_el1().bits() as u32;

        // Perform the common cluster specific operations.
        if previous_state.states[Self::CLUSTER_POWER_LEVEL] == ArmPowerState::Off {
            // This CPU might have woken up whilst the cluster was attempting to power down. In
            // this case the FVP power controller will have a pending cluster power off request
            // which needs to be cleared by writing to the PPONR register. This prevents the power
            // controller from interpreting a subsequent entry of this cpu into a simple wfi as a
            // power down request.
            self.power_controller.lock().power_on_processor(mpidr);
        }

        // Clear PWKUPR.WEN bit to ensure interrupts do not interfere with a cpu power down unless
        // the bit is set again.
        self.power_controller.lock().disable_wakeup_requests(mpidr);
    }
}

impl<'a>
    PowerControllerInterface<
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        { FvpCoreIndex::CORE_COUNT },
        PSCI_NON_CPU_DOMAIN_COUNT,
        u8,
        ArmPowerState,
    > for FvpPsciPowerController<'a>
{
    type Error = FvpPsciPowerControllerError;

    fn try_parse_power_state(power_state: PowerState) -> Option<FvpCompositePowerState> {
        const POWER_LEVEL_STATE_MASK: u32 = 0x0000_0fff;
        const ARM_LOCAL_PSTATE_WIDTH: u32 = 4;
        const ARM_LOCAL_PSTATE_MASK: u32 = (1 << ARM_LOCAL_PSTATE_WIDTH) - 1;
        // last_at_power_level is encoded in the bits immediately following the state ID bits
        // for each power level.
        let last_at_pwr_lvl_shift: u32 = ARM_LOCAL_PSTATE_WIDTH * (PSCI_MAX_POWER_LEVEL as u32 + 1);

        if let PowerState::StandbyOrRetention(0x01) = power_state {
            return Some(PsciCompositePowerState::new([
                ArmPowerState::Retention,
                ArmPowerState::Run,
                ArmPowerState::Run,
            ]));
        }

        let value = match power_state {
            PowerState::PowerDown(v) => v,
            _ => return None,
        };

        let states = match value & POWER_LEVEL_STATE_MASK {
            0x002 => [ArmPowerState::Off, ArmPowerState::Run, ArmPowerState::Run],
            0x022 => [ArmPowerState::Off, ArmPowerState::Off, ArmPowerState::Run],
            // Ensure that the system power domain level is never suspended via PSCI
            // CPU_SUSPEND API. System suspend is only supported via PSCI SYSTEM_SUSPEND
            // API.
            0x222 => [ArmPowerState::Off, ArmPowerState::Off, ArmPowerState::Run],
            _ => return None,
        };

        let last_at_power_level =
            ((value >> last_at_pwr_lvl_shift) & ARM_LOCAL_PSTATE_MASK) as usize;

        if last_at_power_level > PSCI_MAX_POWER_LEVEL {
            return None;
        }

        Some(PsciCompositePowerState::new_with_last_power_level(
            states,
            last_at_power_level,
        ))
    }

    fn power_domain_suspend(
        &self,
        target_state: &FvpCompositePowerState,
    ) -> Result<(), Self::Error> {
        let mpidr = read_mpidr_el1().bits() as u32;

        self.power_controller.lock().enable_wakeup_requests(mpidr);

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == ArmPowerState::Off {
            self.power_controller.lock().power_off_cluster(mpidr);
        }

        self.power_controller.lock().power_off_processor(mpidr);

        Ok(())
    }

    fn power_domain_suspend_finish(
        &self,
        previous_state: &FvpCompositePowerState,
    ) -> Result<(), Self::Error> {
        self.power_domain_on_finish_common(previous_state);

        Ok(())
    }

    fn power_domain_validate_suspend(
        &self,
        _target_state: &FvpCompositePowerState,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn power_domain_off(&self, target_state: &FvpCompositePowerState) -> Result<(), Self::Error> {
        let mpidr = read_mpidr_el1().bits() as u32;
        self.power_controller.lock().power_off_processor(mpidr);

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == ArmPowerState::Off {
            self.power_controller.lock().power_off_cluster(mpidr);
        }

        Ok(())
    }

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), Self::Error> {
        let raw_mpidr: u32 = mpidr
            .try_into()
            .map_err(|err| FvpPsciPowerControllerError(ErrorCode::from(err)))?;

        // Ensure that we do not cancel an inflight power off request for the
        // target cpu. That would leave it in a zombie wfi. Wait for it to power
        // off and then program the power controller to turn that CPU on.
        loop {
            let psysr = self.power_controller.lock().system_status(raw_mpidr);
            if !psysr.contains(SystemStatus::L0) {
                break;
            }
        }

        self.power_controller.lock().power_on_processor(raw_mpidr);

        Ok(())
    }

    fn power_domain_on_finish(&self, previous_state: &FvpCompositePowerState) {
        self.power_domain_on_finish_common(previous_state);
    }

    fn system_off(&self) -> Result<(), Self::Error> {
        self.system
            .lock()
            .write_system_configuration(SystemConfigFunction::Shutdown);

        Ok(())
    }

    fn system_reset(&self) -> Result<(), Self::Error> {
        self.system
            .lock()
            .write_system_configuration(SystemConfigFunction::Reboot);

        Ok(())
    }

    fn node_hw_state(&self, mpidr: Mpidr, power_level: u32) -> Result<HwState, Self::Error> {
        let raw_mpidr: u32 = mpidr
            .try_into()
            .map_err(|err| FvpPsciPowerControllerError(ErrorCode::from(err)))?;

        let status_flag = match power_level as usize {
            CPU_POWER_LEVEL => SystemStatus::L0,
            Self::CLUSTER_POWER_LEVEL => {
                // Use L1 affinity if MPIDR_EL1.MT bit is not set else use L2 affinity.
                if raw_mpidr & 0x1 == 0 {
                    SystemStatus::L1
                } else {
                    SystemStatus::L2
                }
            }
            _ => return Err(FvpPsciPowerControllerError(ErrorCode::InvalidParameters)),
        };

        let psysr = self.power_controller.lock().system_status(raw_mpidr);
        Ok(if psysr.contains(status_flag) {
            HwState::On
        } else {
            HwState::Off
        })
    }

    fn sys_suspend_power_state(&self) -> FvpCompositePowerState {
        PsciCompositePowerState::OFF
    }

    fn is_feature_supported(&self, feature: PsciPlatformOptionalFeatures) -> bool {
        const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::NODE_HW_STATE
            .union(PsciPlatformOptionalFeatures::SYSTEM_SUSPEND)
            .union(PsciPlatformOptionalFeatures::OS_INITIATED_MODE);
        FEATURES.contains(feature)
    }
}
