// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    Fvp, GIC,
    config::{FVP_CLUSTER_COUNT, FVP_MAX_CPUS_PER_CLUSTER},
    map_peripheral,
};
use arm_fvp_base_pac::{
    MemoryMap, PhysicalInstance,
    arm_generic_timer::memory_mapped::{
        CntAcr, CntControlBase, CntCtlBase, GenericTimerControl, GenericTimerCtl,
    },
    power_controller::{FvpPowerController, FvpPowerControllerRegisters, SystemStatus},
    system::{FvpSystemPeripheral, FvpSystemRegisters, SystemConfigFunction},
};
use rf_a_bl31::{
    aarch64::{dsb_ish, wfi},
    platform::Platform,
    reexports::{
        arm_gic::gicv3::{GicDistributorContext, GicRedistributorContext},
        arm_psci::{EntryPoint, ErrorCode, HwState, Mpidr, PowerState},
        arm_sysregs::{
            el0::{accessors::write_cntfrq_el0, registers::CntfrqEl0},
            el1::accessors::read_mpidr_el1,
        },
        log,
        spin::mutex::SpinMutex,
    },
    services::psci::{
        CPU_POWER_LEVEL, PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
        PsciPlatformInterface, PsciPlatformOptionalFeatures,
    },
};

#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum FvpPowerState {
    Run = 0,
    Retention = 1,
    Off = 2,
}

impl PlatformPowerStateInterface for FvpPowerState {
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

impl From<FvpPowerState> for usize {
    fn from(value: FvpPowerState) -> Self {
        value as usize
    }
}

struct FvpGicContext {
    distributor_context: GicDistributorContext<
        { GicDistributorContext::ireg_count(988) },
        { GicDistributorContext::ireg_e_count(1024) },
    >,
    redistributor_context: GicRedistributorContext<{ GicRedistributorContext::ireg_count(96) }>,
}

impl FvpGicContext {
    const fn new() -> Self {
        Self {
            distributor_context: GicDistributorContext::new(),
            redistributor_context: GicRedistributorContext::new(),
        }
    }
}

static GIC_CONTEXT: SpinMutex<FvpGicContext> = SpinMutex::new(FvpGicContext::new());

pub struct FvpPsciPlatformImpl<'a> {
    power_controller: SpinMutex<FvpPowerController<'a>>,
    system: SpinMutex<FvpSystemPeripheral<'a>>,
    timer_control: SpinMutex<GenericTimerControl<'a>>,
    timer_ctl: SpinMutex<GenericTimerCtl<'a>>,
}

impl FvpPsciPlatformImpl<'_> {
    const CLUSTER_POWER_LEVEL: usize = 1;
    const NS_TIMER_INDEX: usize = 1;

    pub fn new(
        power_controller: PhysicalInstance<FvpPowerControllerRegisters>,
        system: PhysicalInstance<FvpSystemRegisters>,
        timer_control: PhysicalInstance<CntControlBase>,
        timer_ctl: PhysicalInstance<CntCtlBase>,
    ) -> Self {
        Self {
            power_controller: SpinMutex::new(FvpPowerController::new(map_peripheral(
                power_controller,
            ))),
            system: SpinMutex::new(FvpSystemPeripheral::new(map_peripheral(system))),
            timer_control: SpinMutex::new(GenericTimerControl::new(map_peripheral(timer_control))),
            timer_ctl: SpinMutex::new(GenericTimerCtl::new(map_peripheral(timer_ctl))),
        }
    }

    fn power_domain_on_finish_common(&self, previous_state: &FvpCompositePowerState) {
        assert_eq!(previous_state.cpu_level_state(), FvpPowerState::Off);

        let mpidr = read_mpidr_el1().bits() as u32;

        // Perform the common cluster specific operations.
        if previous_state.states[Self::CLUSTER_POWER_LEVEL] == FvpPowerState::Off {
            // This CPU might have woken up whilst the cluster was attempting to power down. In
            // this case the FVP power controller will have a pending cluster power off request
            // which needs to be cleared by writing to the PPONR register. This prevents the power
            // controller from interpreting a subsequent entry of this cpu into a simple wfi as a
            // power down request.
            self.power_controller.lock().power_on_processor(mpidr);
        }

        // Perform the common system specific operations.
        if previous_state.highest_level_state() == FvpPowerState::Off {
            self.restore_system_power_domain();
        }

        // Clear PWKUPR.WEN bit to ensure interrupts do not interfere with a cpu power down unless
        // the bit is set again.
        self.power_controller.lock().disable_wakeup_requests(mpidr);

        let frequency = self.timer_control.lock().base_frequency();
        write_cntfrq_el0(CntfrqEl0::from_bits_retain(frequency.into()));
    }

    // Enable and initialize the system level generic timer
    pub fn init_generic_timer(&self) {
        let mut timer_control = self.timer_control.lock();

        timer_control.set_enable(true);

        let frequency = timer_control.base_frequency();

        let mut timer_ctl = self.timer_ctl.lock();

        timer_ctl.set_access_control(Self::NS_TIMER_INDEX, CntAcr::all());
        timer_ctl.set_non_secure_access(Self::NS_TIMER_INDEX, true);
        timer_ctl.set_frequency(frequency);

        write_cntfrq_el0(CntfrqEl0::from_bits_retain(frequency.into()));
    }

    fn save_system_power_domain() {
        let mut context = GIC_CONTEXT.lock();
        let gic = GIC.get().unwrap();

        gic.redistributor_save(&mut context.redistributor_context);
        gic.distributor_save(&mut context.distributor_context);

        log::logger().flush();

        // All the other peripheral which are configured by ARM TF are re-initialized on resume
        // from system suspend. Hence we don't save their state here.
    }

    fn restore_system_power_domain(&self) {
        let context = GIC_CONTEXT.lock();
        let gic = GIC.get().unwrap();

        gic.distributor_restore(&context.distributor_context);
        gic.redistributor_restore(&context.redistributor_context);

        // TODO: plat_arm_security_setup();

        self.init_generic_timer();
    }
}

const _: () = assert!(
    (FVP_CLUSTER_COUNT > 0) && (FVP_CLUSTER_COUNT <= 256),
    "Invalid FVP cluster count"
);

const PSCI_MAX_POWER_LEVEL: usize = 2;
pub const PSCI_STATE_COUNT: usize = PSCI_MAX_POWER_LEVEL + 1;
const PSCI_NON_CPU_DOMAIN_COUNT: usize = 1 + FVP_CLUSTER_COUNT;

type NodeIndex = u8;

type FvpCompositePowerState = PsciCompositePowerState<
    PSCI_STATE_COUNT,
    PSCI_MAX_POWER_LEVEL,
    { Fvp::CORE_COUNT },
    PSCI_NON_CPU_DOMAIN_COUNT,
    NodeIndex,
    FvpPowerState,
>;

impl
    PsciPlatformInterface<
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        { Fvp::CORE_COUNT },
        PSCI_NON_CPU_DOMAIN_COUNT,
    > for FvpPsciPlatformImpl<'_>
{
    const POWER_DOMAIN_COUNT: usize = PSCI_NON_CPU_DOMAIN_COUNT + Fvp::CORE_COUNT;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::NODE_HW_STATE
        .union(PsciPlatformOptionalFeatures::SYSTEM_SUSPEND)
        .union(PsciPlatformOptionalFeatures::OS_INITIATED_MODE);

    type PlatformPowerState = FvpPowerState;

    type NodeIndex = NodeIndex;

    fn topology() -> &'static [usize] {
        const TOPOLOGY: [usize; 2 + FVP_CLUSTER_COUNT] = {
            let mut topology = [0; 2 + FVP_CLUSTER_COUNT];

            topology[0] = 1;
            topology[1] = FVP_CLUSTER_COUNT;

            let mut i = 0;
            loop {
                if i >= FVP_CLUSTER_COUNT {
                    break;
                }
                topology[i + 2] = FVP_MAX_CPUS_PER_CLUSTER;
                i += 1;
            }
            topology
        };

        &TOPOLOGY
    }

    /// Based on 6.5 Recommended StateID Encoding
    fn try_parse_power_state(power_state: PowerState) -> Option<FvpCompositePowerState> {
        const POWER_LEVEL_STATE_MASK: u32 = 0x0000_0fff;
        const ARM_LOCAL_PSTATE_WIDTH: u32 = 4;
        const ARM_LOCAL_PSTATE_MASK: u32 = (1 << ARM_LOCAL_PSTATE_WIDTH) - 1;
        // last_at_power_level is encoded in the bits immediately following the state ID bits
        // for each power level.
        let last_at_pwr_lvl_shift: u32 = ARM_LOCAL_PSTATE_WIDTH * (PSCI_MAX_POWER_LEVEL as u32 + 1);

        if let PowerState::StandbyOrRetention(0x01) = power_state {
            return Some(PsciCompositePowerState::new([
                FvpPowerState::Retention,
                FvpPowerState::Run,
                FvpPowerState::Run,
            ]));
        }

        let value = match power_state {
            PowerState::PowerDown(v) => v,
            _ => return None,
        };

        let states = match value & POWER_LEVEL_STATE_MASK {
            0x002 => [FvpPowerState::Off, FvpPowerState::Run, FvpPowerState::Run],
            0x022 => [FvpPowerState::Off, FvpPowerState::Off, FvpPowerState::Run],
            // Ensure that the system power domain level is never suspended via PSCI
            // CPU_SUSPEND API. System suspend is only supported via PSCI SYSTEM_SUSPEND
            // API.
            0x222 => [FvpPowerState::Off, FvpPowerState::Off, FvpPowerState::Run],
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

    fn cpu_standby(&self, cpu_state: FvpPowerState) {
        assert!(cpu_state.power_state_type() == PowerStateType::StandbyOrRetention);

        // Enter standby state. DSB is good practice before using WFI to enter low power states.
        dsb_ish();
        wfi();
    }

    fn power_domain_suspend(&self, target_state: &FvpCompositePowerState) {
        // FVP has retention only at cpu level. Just return as nothing is to be done for retention.
        if target_state.cpu_level_state() == FvpPowerState::Retention {
            return;
        }

        assert_eq!(target_state.cpu_level_state(), FvpPowerState::Off);

        let mpidr = read_mpidr_el1().bits() as u32;

        self.power_controller.lock().enable_wakeup_requests(mpidr);

        // Prevent interrupts from spuriously waking up this cpu.
        GIC.get().unwrap().cpu_interface_disable();

        // The Redistributor is not powered off as it can potentially prevent wake up events
        // reaching the CPUIF and/or might lead to losing register context.

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == FvpPowerState::Off {
            self.power_controller.lock().power_off_cluster(mpidr);
        }

        // Perform the common system specific operations.
        if target_state.highest_level_state() == FvpPowerState::Off {
            Self::save_system_power_domain();
        }

        self.power_controller.lock().power_off_processor(mpidr);
    }

    fn power_domain_suspend_finish(&self, previous_state: &FvpCompositePowerState) {
        // Nothing to be done on waking up from retention at CPU level.
        if previous_state.cpu_level_state() == FvpPowerState::Retention {
            return;
        }

        self.power_domain_on_finish_common(previous_state);
        GIC.get().unwrap().cpu_interface_enable();
    }

    fn power_domain_off(&self, target_state: &FvpCompositePowerState) {
        assert_eq!(FvpPowerState::Off, target_state.cpu_level_state());

        let gic = GIC.get().unwrap();
        gic.cpu_interface_disable();
        gic.redistributor_off();

        let mpidr = read_mpidr_el1().bits() as u32;
        self.power_controller.lock().power_off_processor(mpidr);

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == FvpPowerState::Off {
            self.power_controller.lock().power_off_cluster(mpidr);
        }
    }

    fn power_domain_power_down(&self, _target_state: &FvpCompositePowerState) {}

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode> {
        let raw_mpidr: u32 = mpidr.try_into().map_err(ErrorCode::from)?;

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
        let gic = GIC.get().unwrap();
        gic.redistributor_init(&Fvp::GIC_CONFIG);
        gic.cpu_interface_enable();
    }

    fn system_off(&self) -> ! {
        self.system
            .lock()
            .write_system_configuration(SystemConfigFunction::Shutdown);
        wfi();
        unreachable!("expected system off did not happen");
    }

    fn system_reset(&self) -> ! {
        self.system
            .lock()
            .write_system_configuration(SystemConfigFunction::Reboot);
        wfi();
        unreachable!("expected system reset did not happen");
    }

    fn node_hw_state(&self, target_cpu: Mpidr, power_level: u32) -> Result<HwState, ErrorCode> {
        let raw_mpidr: u32 = target_cpu.try_into().map_err(ErrorCode::from)?;

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
            _ => return Err(ErrorCode::InvalidParameters),
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

    /// Validates a non-secure entry point, optional.
    fn is_valid_ns_entrypoint(&self, entry: &EntryPoint) -> bool {
        let entrypoint = entry.entry_point_address() as usize;

        MemoryMap::DRAM0.contains(&entrypoint) || MemoryMap::DRAM1.contains(&entrypoint)
    }

    fn power_domain_validate_suspend(
        &self,
        _target_state: &FvpCompositePowerState,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }
}
