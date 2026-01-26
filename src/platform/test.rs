// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{DummyService, Platform};
#[cfg(feature = "rme")]
use crate::Services;
use crate::{
    aarch64::sev,
    context::EntryPointInfo,
    cpu::{Cpu, define_cpu_ops},
    cpu_extensions::CpuExtension,
    dram::const_zeroed,
    errata_framework::{Cve, Erratum, ErratumId, ErratumType, define_errata_list},
    gicv3::{Gic, GicConfig},
    logger::{self, LogSink},
    pagetable::{IdMap, MT_DEVICE, disable_mmu_el3, early_pagetable::define_early_mapping},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures,
        },
        trng::{TrngError, TrngPlatformInterface},
    },
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::{
    IntId, UniqueMmioPointer,
    gicv3::registers::{Gicd, GicrSgi, Waker},
};
use arm_psci::{Cookie, ErrorCode, HwState, Mpidr, PowerState, SystemOff2Type};
use arm_sysregs::{MidrEl1, MpidrEl1};
use core::{fmt, ptr::NonNull};
use percore::ExceptionFree;
use spin::mutex::{SpinMutex, SpinMutexGuard};
use std::io::{Write, stdout};
use uuid::Uuid;
use zerocopy::{FromBytes, transmute_mut};

const DEVICE0_BASE: usize = 0x0200_0000;
const DEVICE0_SIZE: usize = 0x1000;
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);

// The levels of the power topology System, SoC, Cluster, Core.
const SYSTEM_DOMAIN_INDEX: u8 = 0;
const SOCS_PER_SYSTEM: u8 = 2;
const CLUSTERS_PER_SOC: usize = 2;
// Each cluster has 3 cores except the last one which has 4.
const CORES_PER_CLUSTER: usize = 3;
const CORES_PER_CLUSTER_LAST: usize = 4;

define_early_mapping!([]);
define_errata_list!(TestMitigatedErratum, TestUnneededErratum);

static FAKE_GIC: SpinMutex<FakeGic> = SpinMutex::new(const_zeroed());

/// A fake platform for unit tests.
pub struct TestPlatform;

impl TestPlatform {
    /// The MPIDR values for each core, for use in tests.
    pub const MPIDR_VALUES: [MpidrEl1; Self::CORE_COUNT] = [
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0000),
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0001),
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0002),
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0100),
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0101),
        MpidrEl1::from_bits_retain(0x0000_0000_0000_0102),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0000),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0001),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0002),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0100),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0101),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0102),
        MpidrEl1::from_bits_retain(0x0000_0000_0001_0103),
    ];
}

// SAFETY: The test platform is exempt from the usual safety requirements on `core_position`,
// because it is only used in unit tests and so `TestPlatform::core_position` is never called from
// assembly code.
unsafe impl Platform for TestPlatform {
    const CORE_COUNT: usize = 13;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    const PAGE_HEAP_PAGE_COUNT: usize = 6;

    #[cfg(feature = "rme")]
    const RMM_SHARED_BUFFER_START: usize = 0xffbf_f000;

    type LogSinkImpl = StdOutSink;
    type PsciPlatformImpl = TestPsciPlatformImpl;
    type TrngPlatformImpl = TestTrngPlatformImpl;

    type PlatformServiceImpl = DummyService;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] = &[];

    fn init_with_early_mapping(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        logger::init(StdOutSink).expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        // SAFETY: The pagetable isn't actually used in unit tests.
        unsafe {
            idmap.map_region(&DEVICE0, MT_DEVICE);
        }
    }

    unsafe fn create_gic() -> Gic<'static> {
        SpinMutexGuard::leak(FAKE_GIC.try_lock().unwrap()).build()
    }

    #[cfg(feature = "pauth")]
    fn init_apkey() -> u128 {
        0
    }

    fn create_service() -> Self::PlatformServiceImpl {
        DummyService
    }

    fn handle_group0_interrupt(int_id: IntId) {
        panic!("Received group 0 interrupt {int_id:?}")
    }

    fn secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x4000_0000,
            args: Default::default(),
        }
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x6000_0000,
            args: Default::default(),
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0xfdc0_0000,
            args: Services::get().rmmd.entrypoint_args(),
        }
    }

    fn mpidr_is_valid(mpidr: MpidrEl1) -> bool {
        let system_index = mpidr.aff3();
        let soc_index = mpidr.aff2();
        let cluster_index = usize::from(mpidr.aff1());
        let core_index = usize::from(mpidr.aff0());

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

    extern "C" fn core_position(mpidr: u64) -> usize {
        let mpidr = MpidrEl1::from_bits_retain(mpidr);

        assert!(TestPlatform::mpidr_is_valid(mpidr));

        let soc_index = usize::from(mpidr.aff2());
        let cluster_index = usize::from(mpidr.aff1());
        let core_index = usize::from(mpidr.aff0());

        ((soc_index * CLUSTERS_PER_SOC) + cluster_index) * CORES_PER_CLUSTER + core_index
    }

    unsafe extern "C" fn cold_boot_handler() {}

    extern "C" fn crash_console_init() -> u32 {
        1
    }

    extern "C" fn crash_console_putc(_char: u32) -> i32 {
        unimplemented!()
    }

    extern "C" fn crash_console_flush() {}

    unsafe extern "C" fn dump_registers() {}
}

/// A fake GICv3 for unit tests.
#[derive(Clone, Eq, PartialEq, FromBytes)]
struct FakeGic {
    gicd_regs: Gicd,
    gicr_regs: [GicrSgi; TestPlatform::CORE_COUNT],
}

impl FakeGic {
    fn build(&mut self) -> Gic<'_> {
        for (core_index, mpidr) in TestPlatform::MPIDR_VALUES.iter().enumerate() {
            let typer: &mut u64 = transmute_mut!(&mut self.gicr_regs[core_index].gicr.typer.0);
            *typer = u64::from(mpidr.aff3()) << 56
                | u64::from(mpidr.aff2()) << 48
                | u64::from(mpidr.aff1()) << 40
                | u64::from(mpidr.aff0()) << 32;
            if core_index == TestPlatform::CORE_COUNT - 1 {
                // Mark the last redistributor as being the last.
                *typer |= 1 << 4;
            }
            self.gicr_regs[core_index].gicr.waker.0 = Waker::CHILDREN_ASLEEP;
        }

        let gicd = UniqueMmioPointer::from(&mut self.gicd_regs);
        let gicr_base = NonNull::new(self.gicr_regs.as_mut_ptr()).unwrap();
        // SAFETY: The gicr_base pointer comes from a reference to an array of fake registers which
        // we don't otherwise access after this point, and the last entry is marked as such.
        unsafe { Gic::new(gicd, gicr_base, false) }
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

/// A log sink for tests which writes logs to standard output.
pub struct StdOutSink;

impl LogSink for StdOutSink {
    fn write_fmt(&self, args: fmt::Arguments) {
        stdout().write_fmt(args).unwrap();
    }

    fn flush(&self) {
        stdout().flush().unwrap();
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
            PowerState::PowerDown(0x3) => [
                TestPowerState::PowerDown,
                TestPowerState::On,
                TestPowerState::On,
                TestPowerState::On,
            ],
            PowerState::PowerDown(0x23) => [
                TestPowerState::PowerDown,
                TestPowerState::Standby2,
                TestPowerState::On,
                TestPowerState::On,
            ],
            PowerState::PowerDown(0x33) => [
                TestPowerState::PowerDown,
                TestPowerState::PowerDown,
                TestPowerState::On,
                TestPowerState::On,
            ],
            PowerState::PowerDown(0x333) => [
                TestPowerState::PowerDown,
                TestPowerState::PowerDown,
                TestPowerState::PowerDown,
                TestPowerState::On,
            ],

            PowerState::PowerDown(0x3333) => {
                [TestPowerState::PowerDown; TestPsciPlatformImpl::MAX_POWER_LEVEL + 1]
            }

            _ => return None,
        };
        Some(PsciCompositePowerState::new_with_last_power_level(
            states,
            PsciCompositePowerState::new(states)
                .find_highest_non_run_level()
                .unwrap(),
        ))
    }

    fn cpu_standby(&self, _cpu_state: TestPowerState) {}

    fn power_domain_suspend(&self, _target_state: &PsciCompositePowerState) {}

    fn power_domain_suspend_finish(&self, _previous_state: &PsciCompositePowerState) {}

    fn power_domain_validate_suspend(
        &self,
        _target_state: &PsciCompositePowerState,
    ) -> Result<(), ErrorCode> {
        Ok(())
    }

    fn power_domain_off(&self, target_state: &PsciCompositePowerState) {
        assert_eq!(target_state.cpu_level_state(), TestPowerState::PowerDown);
    }

    fn power_domain_power_down_wfi(&self, _target_state: &PsciCompositePowerState) -> ! {
        // SAFETY: `disable_mmu_el3` is safe to call here as it doesn't actually do anything with
        // the MMU in tests.
        unsafe {
            disable_mmu_el3();
        }
        panic!("{}", Self::POWER_DOWN_WFI_MAGIC);
    }

    fn power_domain_on(&self, _mpidr: Mpidr) -> Result<(), ErrorCode> {
        sev();
        Ok(())
    }

    fn power_domain_on_finish(&self, _previous_state: &PsciCompositePowerState) {}

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

pub struct TestTrngPlatformImpl;

impl TrngPlatformInterface for TestTrngPlatformImpl {
    const TRNG_UUID: Uuid = Uuid::from_bytes([
        0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf,
    ]);

    fn get_entropy() -> Result<[u64; Self::REQ_WORDS], TrngError> {
        // For testing purposes, provide an all-ones entropy source.
        // A real platform would implement this using a hardware TRNG.
        Ok([0xFFFF_FFFF_FFFF_FFFF])
    }
}

struct TestCpu;

/// SAFETY: The dummy implementations of `reset_handler` and `dump_registers` simply return without
/// doing anything. For unit tests it is allowed for them not to be naked functions, because they
/// are never called from assembly code.
unsafe impl Cpu for TestCpu {
    const MIDR: MidrEl1 = MidrEl1::empty();

    extern "C" fn reset_handler() {}

    extern "C" fn dump_registers() {}

    fn power_down_level0() {}

    fn power_down_level1() {}
}

define_cpu_ops!(TestCpu);

pub struct TestMitigatedErratum;

// SAFETY: This erratum is only used in unit tests, so the usual requirements on `check` and
// `workaround` don't apply as they aren't called from assembly.
unsafe impl Erratum for TestMitigatedErratum {
    const ID: ErratumId = 7;
    const CVE: Cve = 1234;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    extern "C" fn check() -> bool {
        true
    }

    extern "C" fn workaround() {}
}

pub struct TestUnneededErratum;

// SAFETY: This erratum is only used in unit tests, so the usual requirements on `check` and
// `workaround` don't apply as they aren't called from assembly.
unsafe impl Erratum for TestUnneededErratum {
    const ID: ErratumId = 8;
    const CVE: Cve = 4321;
    const APPLY_ON: ErratumType = ErratumType::Reset;

    extern "C" fn check() -> bool {
        false
    }

    extern "C" fn workaround() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_logging() {
        let writer = StdOutSink;
        writeln!(writer, "hello");
    }
}
