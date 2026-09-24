// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{
    CLUSTER_COUNT, GIC, MAX_CPUS_PER_CLUSTER, Qemu,
    holding_pen::{hold_pen_signal, secondary_cold_boot_setup},
};
use arm_pl061::{PL061, PL061Registers, UniqueMmioPointer};
use core::ptr::NonNull;
use rf_a_bl31::{
    aarch64::{dsb_sy, isb, wfi},
    bl31_warm_entrypoint,
    context::CoresImpl,
    pagetable::disable_mmu_el3,
    platform::Platform,
    reexports::{
        arm_psci::{ErrorCode, Mpidr, PowerState},
        percore::Cores,
        spin::mutex::SpinMutex,
    },
    services::psci::{
        PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
        PsciPlatformInterface, PsciPlatformOptionalFeatures, try_get_cpu_index_by_mpidr,
    },
};

const PSCI_MAX_POWER_LEVEL: usize = 2;
pub const PSCI_STATE_COUNT: usize = PSCI_MAX_POWER_LEVEL + 1;
const PSCI_NON_CPU_DOMAIN_COUNT: usize = CLUSTER_COUNT + 1;

/// Base addresses for GPIO block that controls system off and system reset as described in the
/// [QEMU ARM virt platform docs](https://qemu-project.gitlab.io/qemu/system/arm/virt.html).
/// Addresses taken from C TF-A.
const SECURE_GPIO_ADDR: *mut PL061Registers = 0x090b_0000 as _;

/// Constants for the system off and system reset GPIO indices.
const SECURE_GPIO_SYSTEM_OFF: usize = 0;
const SECURE_GPIO_SYSTEM_RESET: usize = 1;

// SAFETY: `SECURE_GPIO_ADDR` is the base address for the PL061 device and nothing else
// accesses that address range.
static SECURE_GPIO: SpinMutex<PL061> = SpinMutex::new(PL061::new(unsafe {
    UniqueMmioPointer::new(NonNull::new(SECURE_GPIO_ADDR).unwrap())
}));

#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum QemuPowerState {
    On,
    Retention,
    PowerDown,
}

impl PlatformPowerStateInterface for QemuPowerState {
    const OFF: Self = Self::PowerDown;
    const RUN: Self = Self::On;

    fn power_state_type(&self) -> PowerStateType {
        match self {
            Self::PowerDown => PowerStateType::PowerDown,
            Self::Retention => PowerStateType::StandbyOrRetention,
            Self::On => PowerStateType::Run,
        }
    }
}

impl From<QemuPowerState> for usize {
    fn from(_value: QemuPowerState) -> Self {
        todo!()
    }
}

#[derive(PartialEq, Clone, Copy, Eq)]
enum PowerDownKind {
    // For CPU_OFF
    Off,
    // For CPU_SUSPEND
    Suspend,
}

pub struct QemuPsciPlatformImpl {
    per_cpu_powerdown_kinds: [SpinMutex<PowerDownKind>; Qemu::CORE_COUNT],
}

impl QemuPsciPlatformImpl {
    pub fn new() -> Self {
        Self {
            per_cpu_powerdown_kinds: [const { SpinMutex::new(PowerDownKind::Off) };
                Qemu::CORE_COUNT],
        }
    }
}

impl
    PsciPlatformInterface<
        PSCI_STATE_COUNT,
        PSCI_MAX_POWER_LEVEL,
        { Qemu::CORE_COUNT },
        PSCI_NON_CPU_DOMAIN_COUNT,
    > for QemuPsciPlatformImpl
{
    const POWER_DOMAIN_COUNT: usize = PSCI_NON_CPU_DOMAIN_COUNT + Qemu::CORE_COUNT;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::OS_INITIATED_MODE;

    type PlatformPowerState = QemuPowerState;

    type NodeIndex = u8;

    fn topology() -> &'static [usize] {
        &[1, CLUSTER_COUNT, MAX_CPUS_PER_CLUSTER]
    }

    fn try_parse_power_state(
        power_state: PowerState,
    ) -> Option<
        PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    > {
        const POWER_STATES_MASK: u32 = 0x0000_0fff;
        const LOCAL_PSTATE_WIDTH: u32 = 4;
        const LOCAL_PSTATE_MASK: u32 = (1 << LOCAL_PSTATE_WIDTH) - 1;
        // last_at_power_level is encoded in the bits immediately following the state ID bits
        // for each power level.
        let last_at_power_level_shift: u32 = LOCAL_PSTATE_WIDTH * (PSCI_MAX_POWER_LEVEL as u32 + 1);

        let last_at_power_level_mask: u32 = LOCAL_PSTATE_MASK << last_at_power_level_shift;
        let last_at_power_level: u32 =
            (u32::from(power_state) & last_at_power_level_mask) >> last_at_power_level_shift;
        if last_at_power_level as usize > PSCI_MAX_POWER_LEVEL {
            return None;
        }

        let raw_composite_power_states = u32::from(power_state) & POWER_STATES_MASK;

        if let PowerState::StandbyOrRetention(0x1) = power_state {
            return Some(PsciCompositePowerState::new_with_last_power_level(
                [
                    QemuPowerState::Retention,
                    QemuPowerState::On,
                    QemuPowerState::On,
                ],
                last_at_power_level as usize,
            ));
        }

        if let PowerState::StandbyOrRetention(_) = power_state {
            return None;
        }

        let composite_states = match raw_composite_power_states {
            0x2 => [
                QemuPowerState::PowerDown,
                QemuPowerState::On,
                QemuPowerState::On,
            ],
            0x12 => [
                QemuPowerState::PowerDown,
                QemuPowerState::Retention,
                QemuPowerState::On,
            ],
            0x22 => [
                QemuPowerState::PowerDown,
                QemuPowerState::PowerDown,
                QemuPowerState::On,
            ],
            // Ensure that the system power domain can't be powered down by CPU_SUSPEND. Only SYSTEM_SUSPEND can do that.
            0x222 => [
                QemuPowerState::PowerDown,
                QemuPowerState::PowerDown,
                QemuPowerState::On,
            ],
            _ => return None,
        };

        Some(PsciCompositePowerState::new_with_last_power_level(
            composite_states,
            last_at_power_level as usize,
        ))
    }

    fn cpu_standby(&self, cpu_state: QemuPowerState) {
        assert_eq!(
            cpu_state.power_state_type(),
            PowerStateType::StandbyOrRetention
        );

        dsb_sy();
        wfi();
    }

    fn power_domain_validate_suspend(
        &self,
        _target_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }

    fn power_domain_suspend(
        &self,
        _target_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) {
        *self.per_cpu_powerdown_kinds[CoresImpl::<Qemu>::core_index()].lock() =
            PowerDownKind::Suspend;
    }

    fn power_domain_suspend_finish(
        &self,
        _previous_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) {
    }

    fn power_domain_off(
        &self,
        target_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) {
        assert_eq!(target_state.cpu_level_state(), QemuPowerState::PowerDown);

        GIC.get().unwrap().cpu_interface_disable();
        *self.per_cpu_powerdown_kinds[CoresImpl::<Qemu>::core_index()].lock() = PowerDownKind::Off;
    }

    fn power_domain_power_down(
        &self,
        _target_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) {
        if *self.per_cpu_powerdown_kinds[CoresImpl::<Qemu>::core_index()].lock()
            == PowerDownKind::Off
        {
            // SAFETY: `disable_mmu_el3` is safe to call here as the CPU is about to be switched off.
            // `secondary_cold_boot_setup` is trusted assembly.
            unsafe {
                disable_mmu_el3();
                secondary_cold_boot_setup();
            }
        } else {
            dsb_sy();
            wfi();
            // Instead of behaving as if this was a powerdown abandon, simply call the bl31
            // warmboot entry point. This is closer to what real hardware would do most of the time.
            // SAFETY: `bl31_warmboot_entrypoint` and `disable_mmu_el3` are trusted assembly.
            unsafe {
                disable_mmu_el3();
                bl31_warm_entrypoint::<Qemu>();
            }
        }
    }

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode> {
        let cpu_index = try_get_cpu_index_by_mpidr::<Qemu, Self::NodeIndex>(mpidr)
            .ok_or(ErrorCode::InvalidParameters)?;
        debug_assert!(usize::from(cpu_index) < Qemu::CORE_COUNT);
        hold_pen_signal(cpu_index.into(), bl31_warm_entrypoint::<Qemu>);
        Ok(())
    }

    fn power_domain_on_finish(
        &self,
        previous_state: &PsciCompositePowerState<
            PSCI_STATE_COUNT,
            PSCI_MAX_POWER_LEVEL,
            { Qemu::CORE_COUNT },
            PSCI_NON_CPU_DOMAIN_COUNT,
            Self::NodeIndex,
            QemuPowerState,
        >,
    ) {
        assert_eq!(previous_state.cpu_level_state(), QemuPowerState::PowerDown);
        let gic = GIC.get().unwrap();
        gic.redistributor_init(&Qemu::GIC_CONFIG);
        gic.cpu_interface_enable();
    }

    fn system_off(&self) -> ! {
        let mut gpio = SECURE_GPIO.lock();
        gpio.pin_set(SECURE_GPIO_SYSTEM_OFF, false).unwrap();
        gpio.pin_set(SECURE_GPIO_SYSTEM_OFF, true).unwrap();
        isb();
        panic!("System off was not triggered by secure GPIO pin");
    }

    fn system_reset(&self) -> ! {
        let mut gpio = SECURE_GPIO.lock();
        gpio.pin_set(SECURE_GPIO_SYSTEM_RESET, false).unwrap();
        gpio.pin_set(SECURE_GPIO_SYSTEM_RESET, true).unwrap();
        isb();
        panic!("System reset was not triggered by secure GPIO pin");
    }
}

pub fn gpio_init() {
    let mut gpio = SECURE_GPIO.lock();
    let mut config = gpio.config();
    config.into_output(SECURE_GPIO_SYSTEM_OFF).unwrap();
    config.into_output(SECURE_GPIO_SYSTEM_RESET).unwrap();
}
