// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

mod config;

use self::config::{FVP_CLUSTER_COUNT, FVP_MAX_CPUS_PER_CLUSTER, FVP_MAX_PE_PER_CPU};
use super::{DummyService, Platform};
use crate::{
    aarch64::{dsb_ish, dsb_sy, wfi},
    context::{CoresImpl, EntryPointInfo},
    cpu::aem_generic::AemGeneric,
    cpu::define_cpu_ops,
    debug::DEBUG,
    gicv3::GicConfig,
    logger::{self, LockedWriter},
    pagetable::{IdMap, MT_DEVICE, map_region},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures, bl31_warm_entrypoint,
        },
        trng::NotSupportedTrngPlatformImpl,
    },
    sysregs::{IccSre, MpidrEl1, SctlrEl3, Spsr, read_mpidr_el1, read_sctlr_el3, write_cntfrq_el0},
};
use aarch64_paging::paging::{MemoryRegion, VirtualAddress};
use arm_fvp_base_pac::{
    Cci550Map, MemoryMap, Peripherals, PhysicalInstance,
    arm_cci::{Cci5x0, Cci5x0Registers},
    arm_generic_timer::{CntAcr, CntControlBase, CntCtlBase, GenericTimerControl, GenericTimerCtl},
    power_controller::{FvpPowerController, FvpPowerControllerRegisters, SystemStatus},
    system::{FvpSystemPeripheral, FvpSystemRegisters, SystemConfigFunction},
};
use arm_gic::{
    IntId,
    gicv3::{
        GicV3,
        registers::{Gicd, GicrSgi},
    },
};
use arm_pl011_uart::{Uart, UniqueMmioPointer};
use arm_psci::{EntryPoint, ErrorCode, HwState, Mpidr, PowerState};
use core::{
    arch::{global_asm, naked_asm},
    mem::offset_of,
    ptr::NonNull,
};
use percore::Cores;
use spin::mutex::SpinMutex;

const BLD_GIC_VE_MMAP: u32 = 0x0;

/// Base address of GICv3 distributor.
const BASE_GICD_BASE: usize = 0x2f00_0000;
/// Base address of GICv3 redistributor frame.
const BASE_GICR_BASE: usize = 0x2f10_0000;
const VE_GICD_BASE: usize = 0x2c00_1000;

const V2M_SYSREGS_BASE: usize = 0x1c01_0000;
const V2M_SYS_ID: usize = 0x0;
const V2M_SYS_ID_BLD_SHIFT: u32 = 12;

const DEVICE0_BASE: usize = 0x2000_0000;
const DEVICE0_SIZE: usize = 0x0c20_0000;
const DEVICE1_BASE: usize = BASE_GICD_BASE;
const PLATFORM_CORE_COUNT: usize =
    FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;
const DEVICE1_SIZE: usize = (BASE_GICR_BASE - BASE_GICD_BASE) + (PLATFORM_CORE_COUNT * 0x2_0000);
const DEVICE2_BASE: usize = 0x2a00_0000;
const DEVICE2_SIZE: usize = 0x10000;

const ARM_TRUSTED_SRAM_BASE: usize = 0x0400_0000;
const ARM_SHARED_RAM_BASE: usize = ARM_TRUSTED_SRAM_BASE;
const ARM_SHARED_RAM_SIZE: usize = 0x0000_1000; /* 4 KB */

const WARM_ENTRYPOINT_FIELD: *mut unsafe extern "C" fn() =
    ARM_SHARED_RAM_BASE as *mut unsafe extern "C" fn();

const V2M_IOFPGA_BASE: usize = 0x1c00_0000;
const V2M_IOFPGA_SIZE: usize = 0x0300_0000;

const SHARED_RAM: MemoryRegion = MemoryRegion::new(
    ARM_SHARED_RAM_BASE,
    ARM_SHARED_RAM_BASE + ARM_SHARED_RAM_SIZE,
);

const DEVICE_REGIONS: [MemoryRegion; 4] = [
    MemoryRegion::new(V2M_IOFPGA_BASE, V2M_IOFPGA_BASE + V2M_IOFPGA_SIZE),
    MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE),
    MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE),
    MemoryRegion::new(DEVICE2_BASE, DEVICE2_BASE + DEVICE2_SIZE),
];

const V2M_IOFPGA_UART1_BASE: usize = 0x1c0a_0000;

// TODO: These addresses should be parsed from FW_CONFIG
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0x0400_1500;
const NT_FW_CONFIG_ADDRESS: u64 = 0x8000_0000;
const HW_CONFIG_ADDRESS: u64 = 0x07f0_0000;
const HW_CONFIG_ADDRESS_NS: u64 = 0x8200_0000;

// TODO: Use the correct values here (see services/std_svc/rmmd/rmmd_main.c).
/// Version of the RMM Boot Interface.
#[cfg(feature = "rme")]
const RMM_BOOT_VERSION: u64 = 0;
/// Base address for the EL3 - RMM shared area. The boot manifest should be stored at the beginning
/// of this area.
#[cfg(feature = "rme")]
const RMM_SHARED_AREA_BASE_ADDRESS: u64 = 0;

fn device_regions_include<T>(physical_instance: &PhysicalInstance<T>) -> bool {
    let start = physical_instance.pa();
    let end = start + size_of::<T>() - 1;

    DEVICE_REGIONS.iter().any(|region| {
        let range = region.start()..region.end();

        range.contains(&VirtualAddress(start)) && range.contains(&VirtualAddress(end))
    })
}

/// Creates an identity mapped `UniqueMmioPointer` from a `PhysicalInstance`. The function will
/// panic if called with a physical_instance that is not part of the mapped DEVICE_REGIONS.
fn map_peripheral<T>(physical_instance: PhysicalInstance<T>) -> UniqueMmioPointer<'static, T> {
    assert!(device_regions_include(&physical_instance));

    // Safety: Physical instances are unique pointers to peripherals. The addresses remains valid
    // after turning on the MMU because of the identity mapping of the DEVICE_REGIONS.
    unsafe { UniqueMmioPointer::new(NonNull::new(physical_instance.pa() as *mut T).unwrap()) }
}

static FVP_PSCI_PLATFORM_IMPL: SpinMutex<Option<FvpPsciPlatformImpl>> = SpinMutex::new(None);

define_cpu_ops!(AemGeneric);

/// Fixed Virtual Platform
pub struct Fvp;

// SAFETY: `core_position` is indeed a naked function, doesn't access the stack or any other memory,
// only clobbers x0-x5, and returns a unique core index as long as `FVP_MAX_CPUS_PER_CLUSTER` and
// `FVP_MAX_PE_PER_CPU` are correct.
unsafe impl Platform for Fvp {
    const CORE_COUNT: usize = PLATFORM_CORE_COUNT;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    type LogSinkImpl = LockedWriter<Uart<'static>>;
    type PsciPlatformImpl = FvpPsciPlatformImpl<'static>;
    // TODO: Implement TRNG for FVP.
    type TrngPlatformImpl = NotSupportedTrngPlatformImpl;

    type PlatformServiceImpl = DummyService;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    fn init_before_mmu() {
        let peripherals = Peripherals::take().unwrap();

        let uart_pointer = map_peripheral(peripherals.uart0);

        logger::init(LockedWriter::new(Uart::new(uart_pointer)))
            .expect("Failed to initialise logger");

        let psci_platform = FvpPsciPlatformImpl::new(
            peripherals.power_controller,
            peripherals.system,
            peripherals.refclk_cntcontrol,
            peripherals.ap_refclk_cntctl,
            peripherals.cci_550,
        );

        // Safety: At this point the MMU and the caches are off on the primary core and all other
        // cores are off. It is safe to enter coherency.
        unsafe { psci_platform.enter_coherency() };
        psci_platform.init_generic_timer();

        *FVP_PSCI_PLATFORM_IMPL.lock() = Some(psci_platform);

        // Write warm boot entry point the shared memory, so secondary cores can pick it up during
        // boot.
        // Safety: WARM_ENTRYPOINT_FIELD points to a valid, writable address.
        unsafe {
            *WARM_ENTRYPOINT_FIELD = bl31_warm_entrypoint;
        }
        dsb_sy();
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        for region in &DEVICE_REGIONS {
            map_region(idmap, region, MT_DEVICE);
        }
    }

    unsafe fn create_gic() -> GicV3<'static> {
        // SAFETY: `GICD_BASE_ADDRESS` and `GICR_BASE_ADDRESS` are base addresses of a GIC device,
        // and nothing else accesses that address range.
        // TODO: Powering on-off secondary cores will also access their GIC Redistributors.
        unsafe {
            GicV3::new(
                BASE_GICD_BASE as *mut Gicd,
                BASE_GICR_BASE as *mut GicrSgi,
                Fvp::CORE_COUNT,
                false,
            )
        }
    }

    fn create_service() -> Self::PlatformServiceImpl {
        DummyService
    }

    fn handle_group0_interrupt(int_id: IntId) {
        todo!("Handle group0 interrupt {:?}", int_id)
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = CoresImpl::core_index() as u64;
        EntryPointInfo {
            pc: 0x0600_0000,
            #[cfg(feature = "sel2")]
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            #[cfg(not(feature = "sel2"))]
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL1H,
            args: [
                TOS_FW_CONFIG_ADDRESS,
                HW_CONFIG_ADDRESS,
                0,
                0,
                core_linear_id,
                0,
                0,
                0,
            ],
        }
    }

    fn non_secure_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0x8800_0000,
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            args: [NT_FW_CONFIG_ADDRESS, HW_CONFIG_ADDRESS_NS, 0, 0, 0, 0, 0, 0],
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        let core_linear_id = CoresImpl::core_index() as u64;
        EntryPointInfo {
            pc: 0xfdc0_0000,
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            args: [
                core_linear_id,
                RMM_BOOT_VERSION,
                Self::CORE_COUNT as u64,
                RMM_SHARED_AREA_BASE_ADDRESS,
                0,
                0,
                0,
                0,
            ],
        }
    }

    fn mpidr_is_valid(mpidr: MpidrEl1) -> bool {
        if mpidr.contains(MpidrEl1::MT) {
            mpidr.aff3() == 0
                && usize::from(mpidr.aff2()) < FVP_CLUSTER_COUNT
                && usize::from(mpidr.aff1()) < FVP_MAX_CPUS_PER_CLUSTER
                && usize::from(mpidr.aff0()) < FVP_MAX_PE_PER_CPU
        } else {
            mpidr.aff3() == 0
                && mpidr.aff2() == 0
                && usize::from(mpidr.aff1()) < FVP_CLUSTER_COUNT
                && usize::from(mpidr.aff0()) < FVP_MAX_CPUS_PER_CLUSTER
        }
    }

    fn psci_platform() -> Option<Self::PsciPlatformImpl> {
        FVP_PSCI_PLATFORM_IMPL.lock().take()
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

    /// Calculates core linear index as: ClusterId * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU +
    /// CPUId * FVP_MAX_PE_PER_CPU + ThreadId
    #[unsafe(naked)]
    extern "C" fn core_position(mpidr: u64) -> usize {
        naked_asm!(
            // Check for MT bit in MPIDR. If not set, shift MPIDR to left to make it look as if in a
            // multi-threaded implementation.
            "tst	x0, #{MPIDR_MT_MASK}",
            "lsl	x3, x0, #{MPIDR_AFFINITY_BITS}",
            "csel	x3, x3, x0, eq",
            // Extract individual affinity fields from MPIDR.
            "ubfx	x0, x3, #{MPIDR_AFF0_SHIFT}, #{MPIDR_AFFINITY_BITS}",
            "ubfx	x1, x3, #{MPIDR_AFF1_SHIFT}, #{MPIDR_AFFINITY_BITS}",
            "ubfx	x2, x3, #{MPIDR_AFF2_SHIFT}, #{MPIDR_AFFINITY_BITS}",
            // Compute linear position.
            "mov	x4, #{FVP_MAX_CPUS_PER_CLUSTER}",
            "madd	x1, x2, x4, x1",
            "mov	x5, #{FVP_MAX_PE_PER_CPU}",
            "madd	x0, x1, x5, x0",
            "ret",
            MPIDR_MT_MASK = const MpidrEl1::MT.bits(),
            MPIDR_AFF0_SHIFT = const MpidrEl1::AFF0_SHIFT,
            MPIDR_AFF1_SHIFT = const MpidrEl1::AFF1_SHIFT,
            MPIDR_AFF2_SHIFT = const MpidrEl1::AFF2_SHIFT,
            FVP_MAX_CPUS_PER_CLUSTER = const FVP_MAX_CPUS_PER_CLUSTER,
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            FVP_MAX_PE_PER_CPU = const FVP_MAX_PE_PER_CPU,
        );
    }

    #[unsafe(naked)]
    unsafe extern "C" fn cold_boot_handler() {
        naked_asm!("ret");
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_init() -> u32 {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x0, {PLAT_ARM_CRASH_UART_BASE}",
            "mov_imm	x1, {PLAT_ARM_CRASH_UART_CLK_IN_HZ}",
            "mov_imm	x2, {ARM_CONSOLE_BAUDRATE}",
            "b	console_pl011_core_init",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_ARM_CRASH_UART_BASE = const V2M_IOFPGA_UART1_BASE,
            PLAT_ARM_CRASH_UART_CLK_IN_HZ = const 24_000_000,
            ARM_CONSOLE_BAUDRATE = const 115_200,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_putc(char: u32) -> i32 {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x1, {PLAT_ARM_CRASH_UART_BASE}",
            "b	console_pl011_core_putc",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_ARM_CRASH_UART_BASE = const V2M_IOFPGA_UART1_BASE,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_flush() {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x0, {PLAT_ARM_CRASH_UART_BASE}",
            "b	console_pl011_core_flush",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_ARM_CRASH_UART_BASE = const V2M_IOFPGA_UART1_BASE,
        );
    }
}

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

pub struct FvpPsciPlatformImpl<'a> {
    power_controller: SpinMutex<FvpPowerController<'a>>,
    system: SpinMutex<FvpSystemPeripheral<'a>>,
    timer_control: SpinMutex<GenericTimerControl<'a>>,
    timer_ctl: SpinMutex<GenericTimerCtl<'a>>,
    cci550: SpinMutex<Cci5x0<'a>>,
}

impl FvpPsciPlatformImpl<'_> {
    const CLUSTER_POWER_LEVEL: usize = 1;
    const NS_TIMER_INDEX: usize = 1;

    pub fn new(
        power_controller: PhysicalInstance<FvpPowerControllerRegisters>,
        system: PhysicalInstance<FvpSystemRegisters>,
        timer_control: PhysicalInstance<CntControlBase>,
        timer_ctl: PhysicalInstance<CntCtlBase>,
        cci550: PhysicalInstance<Cci5x0Registers>,
    ) -> Self {
        Self {
            power_controller: SpinMutex::new(FvpPowerController::new(map_peripheral(
                power_controller,
            ))),
            system: SpinMutex::new(FvpSystemPeripheral::new(map_peripheral(system))),
            timer_control: SpinMutex::new(GenericTimerControl::new(map_peripheral(timer_control))),
            timer_ctl: SpinMutex::new(GenericTimerCtl::new(map_peripheral(timer_ctl))),
            cci550: SpinMutex::new(Cci5x0::new(map_peripheral(cci550))),
        }
    }

    /// Enter current core to the cache coherency domain.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the master only enables allocating shareable data into its cache
    /// after this function completes.
    pub unsafe fn enter_coherency(&self) {
        // Safety: The function propagates the same safety requirements to the caller.
        unsafe {
            self.cci550
                .lock()
                .add_master_to_coherency(Self::get_cci_master_index());
        }
    }

    /// Remove current core from the cache coherency domain.
    ///
    /// # Safety
    ///
    /// The caller must ensure that the master is configured so that it does not allocate shareable
    /// data into its cache, for example by disabling the data cache. The caller also has to clean
    /// and invalidate all shareable data from the caches in the master prior calling this function.
    unsafe fn exit_coherency(&self) {
        // Safety: The function propagates the same safety requirements to the caller.
        unsafe {
            self.cci550
                .lock()
                .remove_master_from_coherency(Self::get_cci_master_index());
        }
    }

    fn get_cci_master_index() -> usize {
        let mpidr_el1 = read_mpidr_el1();

        match mpidr_el1.aff2() {
            0 => Cci550Map::CLUSTER0,
            1 => Cci550Map::CLUSTER1,
            cluster_index => panic!("Invalid cluster index {cluster_index}"),
        }
    }

    fn cluster_off(&self, mpidr: u32) {
        assert!(!read_sctlr_el3().contains(SctlrEl3::C));

        // Safety: At this point the CPU specific power down function must have turned off the cache
        // on the local core and flushes the cache contents. This is handled by `cpu_power_down`
        // calls in the generic Psci implementation.
        unsafe {
            self.exit_coherency();
        }

        self.power_controller.lock().power_off_cluster(mpidr);
    }

    fn power_domain_on_finish_common(&self, previous_state: &PsciCompositePowerState) {
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

            // Enable coherency if this cluster was off
            // Safety: It is safe to enter coherency, because the platform provides
            // hardware-assisted coherency.
            unsafe {
                self.enter_coherency();
            }
        }

        // Perform the common system specific operations.
        if previous_state.highest_level_state() == FvpPowerState::Off {
            self.restore_system_power_domain();
        }

        // Clear PWKUPR.WEN bit to ensure interrupts do not interfere with a cpu power down unless
        // the bit is set again.
        self.power_controller.lock().disable_wakeup_requests(mpidr);

        let frequency = self.timer_control.lock().base_frequency();
        write_cntfrq_el0(frequency.into());
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

        write_cntfrq_el0(frequency.into());
    }

    fn gic_cpu_interface_enable(&self) {
        // TODO: implement enable_gic_cpu_interface
    }
    fn gic_cpu_interface_disable(&self) {
        // TODO: implement disable_gic_cpu_interface
    }

    fn gic_redistributor_enable(&self) {
        // TODO: implement enable_gic_redistributor
    }

    fn gic_redistributor_disable(&self) {
        // TODO: implement disable_gic_redistributor
    }

    fn save_system_power_domain(&self) {
        // TODO: implement save_system_power_domain
        // plat_arm_gic_save();

        log::logger().flush();

        // All the other peripheral which are configured by ARM TF are re-initialized on resume
        // from system suspend. Hence we don't save their state here.
    }

    fn restore_system_power_domain(&self) {
        // TODO: implement restore_system_power_domain
        // plat_arm_gic_resume();
        // plat_arm_security_setup();
        self.init_generic_timer();
    }
}

const _: () = assert!(
    (FVP_CLUSTER_COUNT > 0) && (FVP_CLUSTER_COUNT <= 256),
    "Invalid FVP cluster count"
);

impl PsciPlatformInterface for FvpPsciPlatformImpl<'_> {
    const POWER_DOMAIN_COUNT: usize =
        1 + FVP_CLUSTER_COUNT + FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER;
    const MAX_POWER_LEVEL: usize = 2;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::NODE_HW_STATE
        .union(PsciPlatformOptionalFeatures::SYSTEM_SUSPEND);

    type PlatformPowerState = FvpPowerState;

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
    fn try_parse_power_state(power_state: PowerState) -> Option<PsciCompositePowerState> {
        const POWER_LEVEL_STATE_MASK: u32 = 0x0000_0fff;

        let states = match power_state {
            PowerState::StandbyOrRetention(0x01) => [
                FvpPowerState::Retention,
                FvpPowerState::Run,
                FvpPowerState::Run,
            ],
            PowerState::PowerDown(power_downstate) => {
                match power_downstate & POWER_LEVEL_STATE_MASK {
                    0x002 => [FvpPowerState::Off, FvpPowerState::Run, FvpPowerState::Run],
                    0x022 => [FvpPowerState::Off, FvpPowerState::Off, FvpPowerState::Run],
                    // Ensure that the system power domain level is never suspended via PSCI
                    // CPU_SUSPEND API. System suspend is only supported via PSCI SYSTEM_SUSPEND
                    // API.
                    0x222 => [FvpPowerState::Off, FvpPowerState::Off, FvpPowerState::Run],
                    _ => return None,
                }
            }
            _ => return None,
        };

        Some(PsciCompositePowerState { states })
    }

    fn cpu_standby(&self, cpu_state: FvpPowerState) {
        assert!(cpu_state.power_state_type() == PowerStateType::StandbyOrRetention);

        // Enter standby state. DSB is good practice before using WFI to enter low power states.
        dsb_ish();
        wfi();
    }

    fn power_domain_suspend(&self, target_state: &PsciCompositePowerState) {
        // FVP has retention only at cpu level. Just return as nothing is to be done for retention.
        if target_state.cpu_level_state() == FvpPowerState::Retention {
            return;
        }

        assert_eq!(target_state.cpu_level_state(), FvpPowerState::Off);

        let mpidr = read_mpidr_el1().bits() as u32;

        self.power_controller.lock().enable_wakeup_requests(mpidr);

        // Prevent interrupts from spuriously waking up this cpu.
        self.gic_cpu_interface_disable();

        // The Redistributor is not powered off as it can potentially prevent wake up events
        // reaching the CPUIF and/or might lead to losing register context.

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == FvpPowerState::Off {
            self.cluster_off(mpidr);
        }

        // Perform the common system specific operations.
        if target_state.highest_level_state() == FvpPowerState::Off {
            self.save_system_power_domain();
        }

        self.power_controller.lock().power_off_processor(mpidr);
    }

    fn power_domain_suspend_finish(&self, previous_state: &PsciCompositePowerState) {
        // Nothing to be done on waking up from retention at CPU level.
        if previous_state.cpu_level_state() == FvpPowerState::Retention {
            return;
        }

        self.power_domain_on_finish_common(previous_state);
        self.gic_cpu_interface_enable();
    }

    fn power_domain_off(&self, target_state: &PsciCompositePowerState) {
        assert_eq!(FvpPowerState::Off, target_state.cpu_level_state());

        self.gic_cpu_interface_disable();
        self.gic_redistributor_disable();

        let mpidr = read_mpidr_el1().bits() as u32;
        self.power_controller.lock().power_off_processor(mpidr);

        if target_state.states[Self::CLUSTER_POWER_LEVEL] == FvpPowerState::Off {
            self.cluster_off(mpidr);
        }
    }

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

    fn power_domain_on_finish(&self, previous_state: &PsciCompositePowerState) {
        self.power_domain_on_finish_common(previous_state);
        self.gic_redistributor_enable();
        self.gic_cpu_interface_enable();
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
            PsciCompositePowerState::CPU_POWER_LEVEL => SystemStatus::L0,
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

    fn sys_suspend_power_state(&self) -> PsciCompositePowerState {
        PsciCompositePowerState::OFF
    }

    /// Validates a non-secure entry point, optional.
    fn is_valid_ns_entrypoint(&self, entry: &EntryPoint) -> bool {
        let entrypoint = entry.entry_point_address() as usize;

        MemoryMap::DRAM0.contains(&entrypoint) || MemoryMap::DRAM1.contains(&entrypoint)
    }
}

global_asm!(
    include_str!("../asm_macros_common.S"),
    include_str!("../arm_macros.S"),
    include_str!("fvp/crash_print_regs.S"),
    include_str!("../arm_macros_purge.S"),
    include_str!("../asm_macros_common_purge.S"),
    DEBUG = const DEBUG as i32,
    ICC_SRE_SRE_BIT = const IccSre::SRE.bits(),
    GICD_ISPENDR = const offset_of!(Gicd, ispendr),
    V2M_SYSREGS_BASE = const V2M_SYSREGS_BASE,
    V2M_SYS_ID = const V2M_SYS_ID,
    V2M_SYS_ID_BLD_SHIFT = const V2M_SYS_ID_BLD_SHIFT,
    BLD_GIC_VE_MMAP = const BLD_GIC_VE_MMAP,
    BASE_GICD_BASE = const BASE_GICD_BASE,
    VE_GICD_BASE = const VE_GICD_BASE,
);
