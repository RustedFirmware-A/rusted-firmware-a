// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::{DummyService, Platform};
use crate::{
    aarch64::{dsb_sy, sev, wfi},
    context::{CoresImpl, EntryPointInfo},
    cpu::{define_cpu_ops, qemu_max::QemuMax},
    cpu_extensions::simd::Simd,
    debug::DEBUG,
    errata_framework::define_errata_list,
    gicv3::{Gic, GicConfig},
    logger::{self, HybridLogger, LockedWriter, inmemory::PerCoreMemoryLogger},
    naked_asm,
    pagetable::{
        IdMap, MT_DEVICE, MT_MEMORY, disable_mmu_el3,
        early_pagetable::{EarlyRegion, define_early_mapping},
    },
    platform::{CpuExtension, plat_my_core_pos},
    semihosting::{AdpStopped, semihosting_exit},
    services::{
        arch::WorkaroundSupport,
        psci::{
            PlatformPowerStateInterface, PowerStateType, PsciCompositePowerState,
            PsciPlatformInterface, PsciPlatformOptionalFeatures, bl31_warm_entrypoint,
            try_get_cpu_index_by_mpidr,
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
use arm_psci::{ErrorCode, Mpidr, PowerState};
use arm_sysregs::{IccSre, MpidrEl1, Spsr};
use core::{arch::global_asm, mem::offset_of, ptr::NonNull};
use percore::Cores;

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

/// Base address of the trusted mailbox.
/// The mailbox has a storage buffer at its base, and a doorbell for each CPU.
/// The size of the mailbox (TRUSTED_MAILBOX_SIZE) is 8 for the buffer plus memory reserved for the
/// doorbells, or holding pens, (HOLD_SIZE) which is equal to Qemu::CORE_COUNT * 8.
const TRUSTED_MAILBOX_BASE: usize = SHARED_RAM_BASE;
/// Location to which to write the address that secondary cores should jump to after being released
/// from their holding pens.
const HOLD_ENTRYPOINT: *mut unsafe extern "C" fn() = TRUSTED_MAILBOX_BASE as _;
/// Base address of hold entries for secondary cores. Writing `HOLD_STATE_GO` to the entry for a
/// secondary core will cause it to be released from its holding pen and jump to `*HOLD_ENTRYPOINT`.
const HOLD_BASE: usize = TRUSTED_MAILBOX_BASE + 8;
const HOLD_ENTRY_SHIFT: u64 = 3;
const HOLD_STATE_WAIT: u64 = 0;
const HOLD_STATE_GO: u64 = 1;

/// Base address of the secure world PL011 UART, aka. UART1.
const UART1_BASE: usize = 0x0904_0000;
const PL011_BASE_ADDRESS: *mut PL011Registers = UART1_BASE as _;
/// Base address of GICv3 distributor.
const GICD_BASE_ADDRESS: *mut Gicd = GICD_BASE as _;
/// Base address of the first GICv3 redistributor frame.
const GICR_BASE_ADDRESS: *mut GicrSgi = GICR_BASE as _;

// TODO: Use the correct addresses here.
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0;
const HW_CONFIG_ADDRESS: u64 = 0;

/// The number of CPU clusters.
const CLUSTER_COUNT: usize = 1;
const PLATFORM_CPU_PER_CLUSTER_SHIFT: usize = 2;
/// The maximum number of CPUs in each cluster.
const MAX_CPUS_PER_CLUSTER: usize = 1 << PLATFORM_CPU_PER_CLUSTER_SHIFT;

/// The per-core log buffer size in bytes.
const LOG_BUFFER_SIZE: usize = 1024;

/// The per-core in-memory logger.
///
/// This is here in a static rather than on the stack because it will be quite large, and we may
/// want to move it to DRAM rather than SRAM.
static MEMORY_LOGGER: PerCoreMemoryLogger<LOG_BUFFER_SIZE> = PerCoreMemoryLogger::new();

define_cpu_ops!(QemuMax);
define_errata_list!();

/// The aarch64 'virt' machine of the QEMU emulator.
pub struct Qemu;

define_early_mapping!([
    EarlyRegion {
        address_range: BL31_BASE..BL32_BASE,
        attributes: MT_MEMORY
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
        HybridLogger<&'static PerCoreMemoryLogger<LOG_BUFFER_SIZE>, LockedWriter<Uart<'static>>>;
    type PsciPlatformImpl = QemuPsciPlatformImpl;
    // QEMU does not have a TRNG.
    type TrngPlatformImpl = NotSupportedTrngPlatformImpl;

    type PlatformServiceImpl = DummyService;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] = &[&Simd];

    fn init_with_early_mapping(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range. The address is valid both with the early mapping and the
        // main one, as it's within the `DEVICE1` region that is identity mapped in both cases.
        let uart_pointer =
            unsafe { UniqueMmioPointer::new(NonNull::new(PL011_BASE_ADDRESS).unwrap()) };
        logger::init(HybridLogger::new(
            &MEMORY_LOGGER,
            LockedWriter::new(Uart::new(uart_pointer)),
        ))
        .expect("Failed to initialise logger");
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

    fn create_service() -> Self::PlatformServiceImpl {
        DummyService
    }

    fn handle_group0_interrupt(int_id: IntId) {
        todo!("Handle group0 interrupt {:?}", int_id)
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = CoresImpl::core_index() as u64;
        EntryPointInfo {
            pc: 0x0e10_0000,
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
            pc: 0x6000_0000,
            spsr: Spsr::D | Spsr::A | Spsr::I | Spsr::F | Spsr::M_AARCH64_EL2H,
            args: Default::default(),
        }
    }

    fn mpidr_is_valid(mpidr: MpidrEl1) -> bool {
        mpidr.aff3() == 0
            && mpidr.aff2() == 0
            && usize::from(mpidr.aff1()) < CLUSTER_COUNT
            && usize::from(mpidr.aff0()) < MAX_CPUS_PER_CLUSTER
    }

    fn psci_platform() -> Option<Self::PsciPlatformImpl> {
        Some(QemuPsciPlatformImpl)
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
            include_str!("../arm_macros.S"),
            "mov_imm x16, {GICD_BASE}",
            "arm_print_gic_regs",
            "ret",
            include_str!("../arm_macros_purge.S"),
            include_str!("../asm_macros_common_purge.S"),
            DEBUG = const DEBUG as i32,
            ICC_SRE_SRE_BIT = const IccSre::SRE.bits(),
            GICD_BASE = const GICD_BASE,
            GICD_ISPENDR = const offset_of!(Gicd, ispendr),
        );
    }
}

#[derive(PartialEq, PartialOrd, Debug, Eq, Ord, Clone, Copy)]
pub enum QemuPowerState {
    PowerDown,
    Standby,
    On,
}

impl PlatformPowerStateInterface for QemuPowerState {
    const OFF: Self = Self::PowerDown;
    const RUN: Self = Self::On;

    fn power_state_type(&self) -> PowerStateType {
        match self {
            Self::PowerDown => PowerStateType::PowerDown,
            Self::Standby => PowerStateType::StandbyOrRetention,
            Self::On => PowerStateType::Run,
        }
    }
}

impl From<QemuPowerState> for usize {
    fn from(_value: QemuPowerState) -> Self {
        todo!()
    }
}

pub struct QemuPsciPlatformImpl;

impl PsciPlatformInterface for QemuPsciPlatformImpl {
    const POWER_DOMAIN_COUNT: usize = 1 + CLUSTER_COUNT + Qemu::CORE_COUNT;
    const MAX_POWER_LEVEL: usize = 2;

    const FEATURES: PsciPlatformOptionalFeatures = PsciPlatformOptionalFeatures::empty();

    type PlatformPowerState = QemuPowerState;

    fn topology() -> &'static [usize] {
        &[1, CLUSTER_COUNT, MAX_CPUS_PER_CLUSTER]
    }

    fn try_parse_power_state(_power_state: PowerState) -> Option<PsciCompositePowerState> {
        todo!()
    }

    fn cpu_standby(&self, cpu_state: QemuPowerState) {
        assert_eq!(cpu_state, QemuPowerState::Standby);

        dsb_sy();
        wfi();
    }

    fn power_domain_suspend(&self, _target_state: &PsciCompositePowerState) {
        todo!()
    }

    fn power_domain_suspend_finish(&self, _previous_state: &PsciCompositePowerState) {
        todo!()
    }

    fn power_domain_off(&self, target_state: &PsciCompositePowerState) {
        assert_eq!(target_state.cpu_level_state(), QemuPowerState::PowerDown);

        Gic::get().cpu_interface_disable();
    }

    fn power_domain_power_down_wfi(&self, _target_state: &PsciCompositePowerState) -> ! {
        // SAFETY: `disable_mmu_el3` is safe to call here as the CPU is about to be switched off.
        // `plat_secondary_cold_boot_setup` is trusted assembly.
        unsafe {
            disable_mmu_el3();
            plat_secondary_cold_boot_setup();
        }
    }

    fn power_domain_on(&self, mpidr: Mpidr) -> Result<(), ErrorCode> {
        let cpu_index = try_get_cpu_index_by_mpidr(mpidr).ok_or(ErrorCode::InvalidParameters)?;
        debug_assert!(cpu_index < Qemu::CORE_COUNT);
        // SAFETY: HOLD_BASE is a valid address and adding cpu_index does not make it go out of
        // bounds of HOLD_BASE + HOLD_SIZE, since cpu_index is guaranteed to be smaller than
        // CORE_COUNT. Additionally, writing the warm boot entry point to the mailbox base address
        // and writing HOLD_STATE_GO to the hold address of the appropriate CPU doesn't violate
        // Rust's safety guarantees, as this memory region is only used for the trusted mailbox.
        unsafe {
            *HOLD_ENTRYPOINT = bl31_warm_entrypoint;
            let cpu_hold_addr = (HOLD_BASE as *mut u64).add(cpu_index);
            *cpu_hold_addr = HOLD_STATE_GO;
        }
        sev();
        Ok(())
    }

    fn power_domain_on_finish(&self, previous_state: &PsciCompositePowerState) {
        assert_eq!(previous_state.cpu_level_state(), QemuPowerState::PowerDown);
        Gic::get().cpu_interface_enable();
    }

    fn system_off(&self) -> ! {
        semihosting_exit(AdpStopped::ApplicationExit, 0);
        panic!("Semihosting system off call unexpectedly returned.");
    }

    fn system_reset(&self) -> ! {
        todo!()
    }
}

/// This function sets up the holding pen mechanism on this core. It waits for an event and then
/// checks the value in the core's holding pen. If the core receives a `HOLD_STATE_GO` signal, it
/// jumps to the location provided in the mailbox (`TRUSTED_MAILBOX_BASE`).
#[unsafe(naked)]
unsafe extern "C" fn plat_secondary_cold_boot_setup() -> ! {
    naked_asm!(
        "bl  {plat_my_core_pos}",
        "lsl x0, x0, #{HOLD_ENTRY_SHIFT}",
        "ldr x2, ={HOLD_BASE}",
    "0:",
        "ldr x1, [x2, x0]",
        "cbz x1, 1f",
        "ldr x1, ={HOLD_STATE_WAIT}",
        "str x1, [x2, x0]",
        "ldr x0, ={TRUSTED_MAILBOX_BASE}",
        "ldr x16, [x0]",
    // x16 is chosen to make this bti c compatible, not just bti j
        "br  x16",
    "1:",
        "wfe",
        "b   0b",
        TRUSTED_MAILBOX_BASE = const SHARED_RAM_BASE,
        HOLD_BASE = const HOLD_BASE,
        HOLD_ENTRY_SHIFT = const HOLD_ENTRY_SHIFT,
        HOLD_STATE_WAIT = const HOLD_STATE_WAIT,
        plat_my_core_pos = sym plat_my_core_pos,
    );
}

global_asm!(include_str!("../arm_macros_data.S"));
