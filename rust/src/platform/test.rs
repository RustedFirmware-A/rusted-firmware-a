// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    context::EntryPointInfo,
    gicv3::GicConfig,
    logger,
    pagetable::{map_region, IdMap, MT_DEVICE},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures,
        },
    },
    sysregs::Spsr,
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::gicv3::GicV3;
use arm_psci::{Cookie, ErrorCode, HwState, Mpidr, PowerState, SystemOff2Type};
use core::fmt;
use percore::ExceptionFree;
use std::io::{stdout, Write};

const DEVICE0_BASE: usize = 0x0200_0000;
const DEVICE0_SIZE: usize = 0x1000;
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);

// The levels of the power topology System, SoC, Cluster, Core.
const SYSTEM_DOMAIN_INDEX: u8 = 0;
const SOCS_PER_SYSTEM: usize = 2;
const CLUSTERS_PER_SOC: usize = 2;
// Each cluster has 3 cores except the last one which has 4.
const CORES_PER_CLUSTER: usize = 3;
const CORES_PER_CLUSTER_LAST: usize = 4;

/// A fake platform for unit tests.
pub struct TestPlatform;

impl Platform for TestPlatform {
    const CORE_COUNT: usize = 13;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    type LoggerWriter = DummyLoggerWriter;
    type PsciPlatformImpl = TestPsciPlatformImpl;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    fn init_before_mmu() {
        logger::init(DummyLoggerWriter {}).expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &DEVICE0, MT_DEVICE);
    }

    unsafe fn create_gic() -> GicV3<'static> {
        unimplemented!();
    }

    fn secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x4000_0000,
            spsr: Spsr::M_AARCH64_EL1T,
            args: Default::default(),
        }
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x6000_0000,
            spsr: Spsr::M_AARCH64_EL1T,
            args: Default::default(),
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x2000_0000,
            spsr: Spsr::M_AARCH64_EL2H,
            args: Default::default(),
        }
    }

    fn mpidr_is_valid(mpidr: Mpidr) -> bool {
        let system_index = mpidr.aff3.unwrap_or(SYSTEM_DOMAIN_INDEX);
        let soc_index = mpidr.aff2 as usize;
        let cluster_index = mpidr.aff1 as usize;
        let core_index = mpidr.aff0 as usize;

        // Validate System, SoC and Cluster indexes
        if system_index != SYSTEM_DOMAIN_INDEX
            || soc_index >= SOCS_PER_SYSTEM
            || cluster_index >= CLUSTERS_PER_SOC
        {
            return false;
        }

        // Validate Core index
        let is_last_cluster =
            soc_index == SOCS_PER_SYSTEM - 1 && cluster_index == CLUSTERS_PER_SOC - 1;
        if is_last_cluster {
            core_index < CORES_PER_CLUSTER_LAST
        } else {
            core_index < CORES_PER_CLUSTER
        }
    }

    fn psci_platform() -> Option<Self::PsciPlatformImpl> {
        Some(TestPsciPlatformImpl::new())
    }

    fn arch_workaround_1_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_1() {}

    fn arch_workaround_2_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_2() {}

    fn arch_workaround_3_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    fn arch_workaround_3() {}

    fn arch_workaround_4_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }
}

/// Runs the given function and returns the result.
///
/// This is a fake version of `percore::exception_free` for use in unit tests only, which must be
/// run on a single thread.
pub fn exception_free<T>(f: impl FnOnce(ExceptionFree) -> T) -> T {
    // SAFETY: This is only used in unit tests, which are run on the host where there are no
    // hardware exceptions nor multiple threads.
    let token = unsafe { ExceptionFree::new() };
    f(token)
}

pub struct DummyLoggerWriter;

impl fmt::Write for DummyLoggerWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut stdout = stdout();
        stdout.write_all(s.as_bytes()).unwrap();
        Ok(())
    }
}

#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum TestPowerState {
    On,
    Standby0,
    Standby1,
    Standby2,
    PowerDown,
}

impl PlatformPowerStateInterface for TestPowerState {
    const OFF: Self = TestPowerState::PowerDown;
    const RUN: Self = TestPowerState::On;

    fn power_state_type(&self) -> PowerStateType {
        match self {
            TestPowerState::PowerDown => PowerStateType::PowerDown,
            TestPowerState::Standby0 | TestPowerState::Standby1 | TestPowerState::Standby2 => {
                PowerStateType::StandbyOrRetention
            }
            TestPowerState::On => PowerStateType::Run,
        }
    }
}

impl From<TestPowerState> for usize {
    fn from(_value: TestPowerState) -> Self {
        todo!()
    }
}

pub struct TestPsciPlatformImpl;

impl TestPsciPlatformImpl {
    // Functions that normally do not return make it impossible to test any PSCI call which ends in
    // these functions. The test platform calls panic with the following magic strings that can be
    // caught by `catch_unwind`. This way the test can expect unwind the calls on power down
    // testing.
    pub const POWER_DOWN_WFI_MAGIC: &str = "POWER_DOWN_WFI_MAGIC";
    pub const SYSTEM_OFF_MAGIC: &str = "SYSTEM_OFF_MAGIC";
    pub const SYSTEM_OFF2_MAGIC: &str = "SYSTEM_OFF2_MAGIC";
    pub const SYSTEM_RESET_MAGIC: &str = "SYSTEM_RESET_MAGIC";
    pub const SYSTEM_RESET2_MAGIC: &str = "SYSTEM_RESET2_MAGIC";
    pub const CPU_FREEZE_MAGIC: &str = "CPU_FREEZE_MAGIC";

    pub fn new() -> Self {
        Self
    }
}

impl PsciPlatformInterface for TestPsciPlatformImpl {
    const POWER_DOMAIN_COUNT: usize = 20;

    const MAX_POWER_LEVEL: usize = 3;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::all();

    type PlatformPowerState = TestPowerState;

    fn topology() -> &'static [usize] {
        &[1, 2, 2, 2, 3, 3, 3, 4]
    }

    fn try_parse_power_state(power_state: PowerState) -> Option<PsciCompositePowerState> {
        let states = match power_state {
            PowerState::StandbyOrRetention(0) => [
                TestPowerState::Standby0,
                TestPowerState::On,
                TestPowerState::On,
                TestPowerState::On,
            ],
            PowerState::StandbyOrRetention(1) => [
                TestPowerState::Standby1,
                TestPowerState::Standby0,
                TestPowerState::On,
                TestPowerState::On,
            ],
            PowerState::StandbyOrRetention(2) => [
                TestPowerState::Standby2,
                TestPowerState::Standby1,
                TestPowerState::Standby0,
                TestPowerState::On,
            ],
            PowerState::PowerDown(0) => {
                [TestPowerState::PowerDown; TestPsciPlatformImpl::MAX_POWER_LEVEL + 1]
            }
            _ => return None,
        };

        Some(PsciCompositePowerState::new(states))
    }

    fn cpu_standby(&self, _cpu_state: TestPowerState) {}

    fn power_domain_suspend(&self, _target_state: &PsciCompositePowerState) {}

    fn power_domain_suspend_finish(&self, _target_state: &PsciCompositePowerState) {}

    fn power_domain_off(&self, _target_state: &PsciCompositePowerState) {}

    fn power_domain_power_down_wfi(&self, _target_state: &PsciCompositePowerState) -> ! {
        panic!("{}", Self::POWER_DOWN_WFI_MAGIC);
    }

    fn power_domain_on(&self, _mpidr: Mpidr) -> Result<(), ErrorCode> {
        Ok(())
    }

    fn power_domain_on_finish(&self, _target_state: &PsciCompositePowerState) {}

    fn system_off(&self) -> ! {
        panic!("{}", Self::SYSTEM_OFF_MAGIC);
    }

    fn system_off2(&self, off_type: SystemOff2Type, cookie: Cookie) -> Result<(), ErrorCode> {
        panic!("{} {:?} {:?}", Self::SYSTEM_OFF2_MAGIC, off_type, cookie);
    }

    fn system_reset(&self) -> ! {
        panic!("{}", Self::SYSTEM_RESET_MAGIC);
    }

    fn system_reset2(
        &self,
        _reset_type: arm_psci::ResetType,
        _cookie: Cookie,
    ) -> Result<(), ErrorCode> {
        panic!("{}", Self::SYSTEM_RESET2_MAGIC);
    }

    fn mem_protect(&self, _enabled: bool) -> Result<bool, ErrorCode> {
        Ok(true)
    }

    fn mem_protect_check_range(&self, _range: arm_psci::MemProtectRange) -> Result<(), ErrorCode> {
        Ok(())
    }

    fn cpu_freeze(&self) -> ! {
        panic!("{}", Self::CPU_FREEZE_MAGIC);
    }

    fn cpu_default_suspend_power_state(&self) -> PowerState {
        PowerState::StandbyOrRetention(0)
    }

    fn node_hw_state(&self, _mpidr: Mpidr, _power_level: u32) -> Result<HwState, ErrorCode> {
        Ok(HwState::Off)
    }

    fn sys_suspend_power_state(&self) -> PsciCompositePowerState {
        PsciCompositePowerState::OFF
    }
}

#[unsafe(no_mangle)]
extern "C" fn plat_calc_core_pos(mpidr: u64) -> usize {
    let mpidr = Mpidr::from_register_value(mpidr);

    assert!(TestPlatform::mpidr_is_valid(mpidr));

    let soc_index = mpidr.aff2 as usize;
    let cluster_index = mpidr.aff1 as usize;
    let core_index = mpidr.aff0 as usize;

    ((soc_index * CLUSTERS_PER_SOC) + cluster_index) * CORES_PER_CLUSTER + core_index
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;

    #[test]
    fn test_basic_logging() {
        let mut writer = DummyLoggerWriter {};
        writeln!(writer, "hello").unwrap();
    }
}
