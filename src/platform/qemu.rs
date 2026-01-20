// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{DummyService, Platform};
use crate::{
    aarch64::{dsb_sy, isb, sev, wfi},
    bl31_warm_entrypoint,
    context::{CoresImpl, EntryPointInfo},
    cpu::{define_cpu_ops, qemu_max::QemuMax},
    cpu_extensions::simd::Simd,
    debug::DEBUG,
    dram::zeroed_mut,
    errata_framework::define_errata_list,
    gicv3::{Gic, GicConfig},
    logger::{
        HybridLogger, LOGGER, LockedWriter,
        inmemory::{MemoryLogger, PerCoreMemoryLogger},
    },
    naked_asm,
    pagetable::{
        IdMap, MT_DEVICE, MT_MEMORY_EL3, disable_mmu_el3,
        early_pagetable::{EarlyRegion, define_early_mapping},
    },
    platform::{CpuExtension, my_core_pos},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures, try_get_cpu_index_by_mpidr,
        },
        trng::NotSupportedTrngPlatformImpl,
    },
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::{
    IntId,
    gicv3::registers::{Gicd, GicrSgi},
};
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use arm_pl061::{PL061, PL061Registers};
use arm_psci::{ErrorCode, Mpidr, PowerState};
#[cfg(feature = "pauth")]
use arm_sysregs::read_cntpct_el0;
use arm_sysregs::{IccSreEl3, MpidrEl1};
#[cfg(feature = "pauth")]
use core::arch::asm;
use core::{arch::global_asm, mem::offset_of, ptr::NonNull};
use percore::Cores;
use spin::mutex::{SpinMutex, SpinMutexGuard};

#[cfg(feature = "rme")]
compile_error!("RME is not supported on QEMU");

const DEVICE0_BASE: usize = 0x0800_0000;
const DEVICE0_SIZE: usize = 0x0100_0000;
const DEVICE1_BASE: usize = 0x0900_0000;
const DEVICE1_SIZE: usize = 0x00c0_0000;
const SEC_SRAM_BASE: usize = 0x0e00_0000;
const SHARED_RAM_BASE: usize = SEC_SRAM_BASE;
const SHARED_RAM_SIZE: usize = 0x0000_1000;
const SHARED_RAM: MemoryRegion =
    MemoryRegion::new(SHARED_RAM_BASE, SHARED_RAM_BASE + SHARED_RAM_SIZE);
const DEVICE0: MemoryRegion = MemoryRegion::new(DEVICE0_BASE, DEVICE0_BASE + DEVICE0_SIZE);
const DEVICE1: MemoryRegion = MemoryRegion::new(DEVICE1_BASE, DEVICE1_BASE + DEVICE1_SIZE);
const BL31_BASE: usize = 0x0e09_0000;
const BL32_BASE: usize = 0x0e10_0000;

const GICD_BASE: usize = 0x0800_0000;
const GICR_BASE: usize = 0x080A_0000;

const HOLD_MAGIC1: u64 = 0xCAFE_CAFE;
const HOLD_MAGIC2: u64 = 0xBEEF_BEEF;
const HOLD_STATE_WAIT: u64 = !0;

#[repr(C, align(64))]
struct HoldSlot {
    entry: u64,
    magic1: u64,
    magic2: u64,
}

const TRUSTED_MAILBOX_BASE: usize = SHARED_RAM_BASE;
const HOLD_SLOTS: *mut [HoldSlot; Qemu::CORE_COUNT] = TRUSTED_MAILBOX_BASE as _;

/// Initialise the hold pen by writing magic tags to every slot.
fn plat_hold_pen_init() {
    // SAFETY: `TRUSTED_MAILBOX_BASE` is the base address of the shared mailbox device memory.
    // Other cores are concurrently reading from this region, but aligned 64bit writes are
    // 'single-copy atomic' as they will either complete in full or not at all.
    unsafe {
        for i in 0..Qemu::CORE_COUNT {
            let slot_ptr = &raw mut (*HOLD_SLOTS)[i];

            (&raw mut (*slot_ptr).entry).write_volatile(HOLD_STATE_WAIT);

            // Ensure the entry value is committed before the magic
            // tags that make this slot visible to polling secondaries.
            core::arch::asm!("dmb sy");

            (&raw mut (*slot_ptr).magic1).write_volatile(HOLD_MAGIC1);
            (&raw mut (*slot_ptr).magic2).write_volatile(HOLD_MAGIC2);
        }
    }
}

/// Signal a secondary core to branch to the given entrypoint.
fn plat_hold_pen_signal(cpu_index: usize, entrypoint: unsafe extern "C" fn() -> !) {
    // SAFETY: `TRUSTED_MAILBOX_BASE` is the base address of the shared mailbox device memory.
    // Other cores are concurrently reading from or writing to this region, but aligned 64bit writes
    // are 'single-copy atomic' as they will either complete in full or not at all.
    unsafe {
        let slot_ptr = &raw mut (*HOLD_SLOTS)[cpu_index];
        (&raw mut (*slot_ptr).entry).write_volatile(entrypoint as usize as u64);
    }

    // Ensure that the entry value is committed before signalling secondary cores to wake up.
    dsb_sy();

    // Signal the secondary core to wake up and jump to the given entrypoint.
    sev();
}

/// Base address of the secure world PL011 UART, aka. UART1.
const UART1_BASE: usize = 0x0904_0000;
const PL011_BASE_ADDRESS: *mut PL011Registers = UART1_BASE as _;
/// Base address of GICv3 distributor.
const GICD_BASE_ADDRESS: *mut Gicd = GICD_BASE as _;
/// Base address of the first GICv3 redistributor frame.
const GICR_BASE_ADDRESS: *mut GicrSgi = GICR_BASE as _;

/// Base addresses for GPIO block that controls system off and system reset as described in the
/// [QEMU ARM virt platform docs](https://qemu-project.gitlab.io/qemu/system/arm/virt.html).
/// Addresses taken from C TF-A.
const SECURE_GPIO_ADDR: *mut PL061Registers = 0x090b_0000 as _;

/// Constants for the system off and system reset GPIO indices.
const SECURE_GPIO_SYSTEM_OFF: usize = 0;
const SECURE_GPIO_SYSTEM_RESET: usize = 1;

/// The address of the Flattened Device Tree Blob (DTB) in RAM.
///
/// The QEMU virt platform
/// [loads it at the start of RAM](https://www.qemu.org/docs/master/system/arm/virt.html#hardware-configuration-information-for-bare-metal-programming).
const DTB_ADDRESS: u64 = 0x4000_0000;

// TODO: Use the correct addresses here.
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0;
const HW_CONFIG_ADDRESS: u64 = 0;

/// The number of CPU clusters.
const CLUSTER_COUNT: usize = 1;
const PLATFORM_CPU_PER_CLUSTER_SHIFT: usize = 2;
/// The maximum number of CPUs in each cluster.
const MAX_CPUS_PER_CLUSTER: usize = 1 << PLATFORM_CPU_PER_CLUSTER_SHIFT;

/// The per-core log buffer size in bytes. We subtract the size of the metadata so that the total
/// size of each `MemoryLogger` will be 1024 bytes.
const LOG_BUFFER_SIZE: usize = 1024 - size_of::<MemoryLogger<0>>();

zeroed_mut! {
    /// Per-core in-memory loggers.
    MEMORY_LOGGERS, [MemoryLogger<LOG_BUFFER_SIZE>; Qemu::CORE_COUNT], unsafe(link_section = ".bss2.dram")
}

define_cpu_ops!(QemuMax);
define_errata_list!();

// SAFETY: `SECURE_GPIO_ADDR` is the base address for the PL061 device and nothing else
// accesses that address range.
static SECURE_GPIO: SpinMutex<PL061> = SpinMutex::new(PL061::new(unsafe {
    UniqueMmioPointer::new(NonNull::new(SECURE_GPIO_ADDR).unwrap())
}));

/// The aarch64 'virt' machine of the QEMU emulator.
pub struct Qemu;

define_early_mapping!([
    EarlyRegion {
        address_range: BL31_BASE..BL32_BASE,
        attributes: MT_MEMORY_EL3
    },
    EarlyRegion {
        address_range: DEVICE1_BASE..(DEVICE1_BASE + DEVICE1_SIZE),
        attributes: MT_DEVICE
    }
]);

// SAFETY: `core_position` is indeed a naked function, doesn't access the stack or any other memory,
// only clobbers x0 and x1, and returns a unique index as long as `PLATFORM_CPU_PER_CLUSTER_SHIFT`
// is correct.
unsafe impl Platform for Qemu {
    const CORE_COUNT: usize = CLUSTER_COUNT * MAX_CPUS_PER_CLUSTER;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    type LogSinkImpl =
        HybridLogger<PerCoreMemoryLogger<'static, LOG_BUFFER_SIZE>, LockedWriter<Uart<'static>>>;
    type PsciPlatformImpl = QemuPsciPlatformImpl;
    // QEMU does not have a TRNG.
    type TrngPlatformImpl = NotSupportedTrngPlatformImpl;

    type PlatformServiceImpl = DummyService;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] = &[&Simd::sve(512, false)];

    fn init_with_early_mapping(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range. The address is valid both with the early mapping and the
        // main one, as it's within the `DEVICE1` region that is identity mapped in both cases.
        let uart_pointer =
            unsafe { UniqueMmioPointer::new(NonNull::new(PL011_BASE_ADDRESS).unwrap()) };
        LOGGER
            .init(HybridLogger::new(
                PerCoreMemoryLogger::new(SpinMutexGuard::leak(MEMORY_LOGGERS.lock()).each_mut()),
                LockedWriter::new(Uart::new(uart_pointer)),
            ))
            .expect("Failed to initialise logger");
    }

    fn init(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        let mut gpio = SECURE_GPIO.lock();
        let mut config = gpio.config();
        config.into_output(SECURE_GPIO_SYSTEM_OFF).unwrap();
        config.into_output(SECURE_GPIO_SYSTEM_RESET).unwrap();
        // Initialize hold pen for all secondary cores
        plat_hold_pen_init();
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        // SAFETY: Nothing is being unmapped, and the regions being mapped have the correct
        // attributes.
        unsafe {
            idmap.map_region(&SHARED_RAM, MT_DEVICE);
            idmap.map_region(&DEVICE0, MT_DEVICE);
            idmap.map_region(&DEVICE1, MT_DEVICE);
        }
    }

    unsafe fn create_gic() -> Gic<'static> {
        // Safety: `GICD_BASE_ADDRESS` is a unique pointer to the Qemu's GICD register block.
        let gicd = unsafe { UniqueMmioPointer::new(NonNull::new(GICD_BASE_ADDRESS).unwrap()) };
        let gicr_base = NonNull::new(GICR_BASE_ADDRESS).unwrap();

        // Safety: `gicr_base` points to a continuously mapped GIC redistributor memory area until
        // the last redistributor block. There are no other references to this address range.
        unsafe { Gic::new(gicd, gicr_base, false) }
    }

    // This is only a toy implementation to generate a seemingly random 128-bit key from FP, LR and
    // cntpct_el0 values. A production system must re-implement this function to generate keys from
    // a reliable entropy source.
    #[cfg(feature = "pauth")]
    fn init_apkey() -> u128 {
        let return_addr: u64;
        let frame_addr: u64;
        let cntpct = read_cntpct_el0().physicalcount();

        // SAFETY: We are just reading general purpose registers.
        unsafe {
            asm!("mov {0}, x30", out(reg) return_addr, options(nomem, nostack, preserves_flags));
            asm!("mov {0}, x29", out(reg) frame_addr, options(nomem, nostack, preserves_flags));
        }

        let key_lo = (return_addr << 13) ^ frame_addr ^ cntpct;
        let key_hi = (frame_addr << 15) ^ return_addr ^ cntpct;

        ((key_hi as u128) << 64) | (key_lo as u128)
    }

    fn create_service() -> Self::PlatformServiceImpl {
        DummyService
    }

    fn handle_group0_interrupt(int_id: IntId) {
        todo!("Handle group0 interrupt {:?}", int_id)
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = CoresImpl::<Self>::core_index() as u64;
        EntryPointInfo {
            pc: 0x0e10_0000,
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
            pc: 0x6000_0000,
            args: [DTB_ADDRESS, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    fn mpidr_is_valid(mpidr: MpidrEl1) -> bool {
        mpidr.aff3() == 0
            && mpidr.aff2() == 0
            && usize::from(mpidr.aff1()) < CLUSTER_COUNT
            && usize::from(mpidr.aff0()) < MAX_CPUS_PER_CLUSTER
    }

    fn psci_platform() -> Option<Self::PsciPlatformImpl> {
        Some(QemuPsciPlatformImpl {
            per_cpu_powerdown_kinds: [const { SpinMutex::new(PowerDownKind::Off) };
                Qemu::CORE_COUNT],
        })
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

    #[unsafe(naked)]
    extern "C" fn core_position(mpidr: u64) -> usize {
        naked_asm!(
            "and	x1, x0, #{MPIDR_CPU_MASK}",
            "and	x0, x0, #{MPIDR_CLUSTER_MASK}",
            "add	x0, x1, x0, LSR #({MPIDR_AFFINITY_BITS} - {PLATFORM_CPU_PER_CLUSTER_SHIFT})",
            "ret",
            MPIDR_CPU_MASK = const MpidrEl1::AFF0_MASK << MpidrEl1::AFF0_SHIFT,
            MPIDR_CLUSTER_MASK = const MpidrEl1::AFF1_MASK << MpidrEl1::AFF1_SHIFT,
            MPIDR_AFFINITY_BITS = const MpidrEl1::AFFINITY_BITS,
            PLATFORM_CPU_PER_CLUSTER_SHIFT = const PLATFORM_CPU_PER_CLUSTER_SHIFT,
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
            "mov_imm	x0, {PLAT_QEMU_CRASH_UART_BASE}",
            "mov_imm	x1, {PLAT_QEMU_CRASH_UART_CLK_IN_HZ}",
            "mov_imm	x2, {PLAT_QEMU_CONSOLE_BAUDRATE}",
            "b	console_pl011_core_init",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_QEMU_CRASH_UART_BASE = const UART1_BASE,
            PLAT_QEMU_CRASH_UART_CLK_IN_HZ = const 1,
            PLAT_QEMU_CONSOLE_BAUDRATE = const 115_200,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_putc(char: u32) -> i32 {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x1, {PLAT_QEMU_CRASH_UART_BASE}",
            "b	console_pl011_core_putc",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_QEMU_CRASH_UART_BASE = const UART1_BASE,
        );
    }

    #[unsafe(naked)]
    extern "C" fn crash_console_flush() {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            "mov_imm	x0, {PLAT_QEMU_CRASH_UART_BASE}",
            "b	console_pl011_core_flush",
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            PLAT_QEMU_CRASH_UART_BASE = const UART1_BASE,
        );
    }

    /// Dumps relevant GIC and CCI registers.
    ///
    /// Clobbers x0-x11, x16, x17, sp.
    #[unsafe(naked)]
    unsafe extern "C" fn dump_registers() {
        naked_asm!(
            include_str!("../asm_macros_common.S"),
            include_str!("../gic_debug_macros.S"),
            "mov_imm x16, {GICD_BASE}",
            "arm_print_gic_regs",
            "ret",
            include_str!("../gic_debug_macros_purge.S"),
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            ICC_SRE_SRE_BIT = const IccSreEl3::SRE.bits(),
            GICD_BASE = const GICD_BASE,
            GICD_ISPENDR = const offset_of!(Gicd, ispendr),
        );
    }
}

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

pub const PSCI_MAX_POWER_LEVEL: usize = 2;
const PSCI_STATE_COUNT: usize = PSCI_MAX_POWER_LEVEL + 1;
const PSCI_NON_CPU_DOMAIN_COUNT: usize = CLUSTER_COUNT + 1;

pub struct QemuPsciPlatformImpl {
    per_cpu_powerdown_kinds: [SpinMutex<PowerDownKind>; Qemu::CORE_COUNT],
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

        Gic::get().cpu_interface_disable();
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
            // `plat_secondary_cold_boot_setup` is trusted assembly.
            unsafe {
                disable_mmu_el3();
                plat_secondary_cold_boot_setup();
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
        plat_hold_pen_signal(cpu_index.into(), bl31_warm_entrypoint::<Qemu>);
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
        Gic::get().redistributor_init(&Qemu::GIC_CONFIG);
        Gic::get().cpu_interface_enable();
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

/// Polls the holding pen for the given core until the magic tags and entrypoint are set, then
/// jumps to the entrypoint. This is called by secondary cores after waking up from a power-down
/// state.
#[unsafe(naked)]
unsafe extern "C" fn plat_secondary_cold_boot_setup() -> ! {
    naked_asm!(
        "bl  {plat_my_core_pos}",
        // x0 = core index
        "mov x1, #{HOLD_SLOT_SIZE}",
        "ldr x2, ={HOLD_SLOTS_BASE}",
        "madd x2, x0, x1, x2", // x2 = HOLD_SLOTS_BASE + core_pos * HOLD_SLOT_SIZE
    "0:",
        "ldr x0, [x2, #{MAGIC1_OFFSET}]", // load magic1
        "ldr x1, ={HOLD_MAGIC1}",
        "cmp x0, x1",
        "b.ne 1f",

        "ldr x0, [x2, #{MAGIC2_OFFSET}]", // load magic2
        "ldr x1, ={HOLD_MAGIC2}",
        "cmp x0, x1",
        "b.ne 1f",

        // Ensure that the loads above are totally completed before we load the entrypoint.
        // This prevents the pipeline from speculatively pulling a stale 'entry' value.
        "dmb sy",

        "ldr x16, [x2, #{ENTRY_OFFSET}]", // load entry
        "ldr x1, ={HOLD_STATE_WAIT}",
        "cmp x16, x1",
        "b.eq 1f",

        // Prevent reuse of stale entry
        "str x1, [x2, #{ENTRY_OFFSET}]", // reset to wait
        // x16 is chosen to make this bti c compatible, not just bti j
        "br x16",
    "1:",
        "wfe",
        "b 0b",
        HOLD_SLOT_SIZE = const core::mem::size_of::<HoldSlot>(),
        ENTRY_OFFSET = const offset_of!(HoldSlot, entry),
        MAGIC1_OFFSET = const offset_of!(HoldSlot, magic1),
        MAGIC2_OFFSET = const offset_of!(HoldSlot, magic2),
        HOLD_SLOTS_BASE = const TRUSTED_MAILBOX_BASE,
        HOLD_MAGIC1 = const HOLD_MAGIC1,
        HOLD_MAGIC2 = const HOLD_MAGIC2,
        HOLD_STATE_WAIT = const HOLD_STATE_WAIT,
        plat_my_core_pos = sym my_core_pos::<Qemu>,
    );
}

global_asm!(include_str!("../gic_debug_macros_data.S"));
