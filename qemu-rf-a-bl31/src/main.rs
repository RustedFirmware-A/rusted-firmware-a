// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RF-A BL31 deployment for QEMU.

#![no_main]
#![no_std]

mod holding_pen;
mod psci;

use self::{
    holding_pen::hold_pen_init,
    psci::{PSCI_STATE_COUNT, QemuPsciPlatformImpl, gpio_init},
};
use arm_pl011_uart::{PL011Registers, Uart, UniqueMmioPointer};
use core::{mem::offset_of, ptr::NonNull};
use rf_a_bl31::{
    all_asm, asm_macros_common, asm_macros_common_purge,
    context::{CoresImpl, EntryPointInfo},
    cpu::qemu_max::QemuMax,
    cpu_extensions::{
        CpuExtension, csv2_2::Csv2_2, gcs::Gcs, hcx::Hcx, sctlr2::Sctlr2, simd::Simd,
    },
    crash_console::pl011::Pl011CrashConsole,
    debug::DEBUG,
    define_cpu_ops, define_errata_list,
    dram::zeroed_mut,
    gic_debug_macros, gic_debug_macros_purge,
    gicv3::{Gic, GicConfig},
    logger::{
        HybridLogger, LockedWriter,
        inmemory::{MemoryLogger, PerCoreMemoryLogger},
    },
    naked_asm,
    pagetable::{
        IdMap, MT_DEVICE, MT_MEMORY_EL3,
        early_pagetable::{EarlyRegion, define_early_mapping},
    },
    panic_handler,
    platform::Platform,
    reexports::{
        aarch64_paging::paging::MemoryRegion,
        arm_gic::{
            IntId,
            gicv3::registers::{Gicd, GicrSgi},
        },
        arm_sysregs::{el1::registers::MpidrEl1, el3::registers::IccSreEl3},
        percore::Cores,
        spin::mutex::SpinMutexGuard,
    },
    services::{
        Service,
        arch::{Arch, ArchPlatform},
        errata_management::ErrataManagement,
        psci::PsciPlatformInterface,
    },
    statics,
};

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

const TRUSTED_MAILBOX_BASE: usize = SHARED_RAM_BASE;

/// Base address of the secure world PL011 UART, aka. UART1.
const UART1_BASE: usize = 0x0904_0000;
const PL011_BASE_ADDRESS: *mut PL011Registers = UART1_BASE as _;
/// Base address of GICv3 distributor.
const GICD_BASE_ADDRESS: *mut Gicd = GICD_BASE as _;
/// Base address of the first GICv3 redistributor frame.
const GICR_BASE_ADDRESS: *mut GicrSgi = GICR_BASE as _;

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

/// The aarch64 'virt' machine of the QEMU emulator.
struct Qemu;

define_cpu_ops!(Qemu, [QemuMax]);
define_errata_list!(Qemu, []);

define_early_mapping!(
    Qemu,
    [
        EarlyRegion {
            address_range: BL31_BASE..BL32_BASE,
            attributes: MT_MEMORY_EL3
        },
        EarlyRegion {
            address_range: DEVICE1_BASE..(DEVICE1_BASE + DEVICE1_SIZE),
            attributes: MT_DEVICE
        }
    ]
);

statics!(Qemu);
all_asm!(Qemu);
panic_handler!();

struct ArchPlatformImpl;

impl ArchPlatform for ArchPlatformImpl {}

static ARCH: Arch<ArchPlatformImpl> = Arch::new();
static ERRATA_MANAGEMENT: ErrataManagement<Qemu> = ErrataManagement::new();

static PLATFORM_SERVICES: [&'static dyn Service; 2] = [&ARCH, &ERRATA_MANAGEMENT];

// SAFETY: `core_position` is indeed a naked function, doesn't access the stack or any other memory,
// only clobbers x0 and x1, and returns a unique index as long as `PLATFORM_CPU_PER_CLUSTER_SHIFT`
// is correct.
unsafe impl Platform for Qemu {
    const CORE_COUNT: usize = CLUSTER_COUNT * MAX_CPUS_PER_CLUSTER;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    type LogSinkImpl = HybridLogger<
        PerCoreMemoryLogger<'static, { Self::CORE_COUNT }, LOG_BUFFER_SIZE, Self>,
        LockedWriter<Uart<'static>>,
    >;
    type IdMap = IdMap<{ Self::PAGE_HEAP_PAGE_COUNT }>;
    type PsciPlatformImpl = QemuPsciPlatformImpl;
    type CrashConsoleImpl = Pl011CrashConsole<UART1_BASE, 1, 115_200>;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] =
        &[&Csv2_2, &Gcs, &Hcx, &Simd::sve(512, false), &Sctlr2];

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
        gpio_init();
        // Initialise hold pen for all secondary cores.
        hold_pen_init();

        GIC.call_once(|| {
            // SAFETY: `GICD_BASE_ADDRESS` is a unique pointer to the Qemu's GICD register block.
            let gicd = unsafe { UniqueMmioPointer::new(NonNull::new(GICD_BASE_ADDRESS).unwrap()) };
            let gicr_base = NonNull::new(GICR_BASE_ADDRESS).unwrap();
            // SAFETY: `gicr_base` points to a continuously mapped GIC redistributor memory area
            // until the last redistributor block. There are no other references to this address
            // range.
            unsafe { Gic::new(gicd, gicr_base).unwrap() }
        });
    }

    fn map_extra_regions(idmap: &mut Self::IdMap) {
        // SAFETY: Nothing is being unmapped, and the regions being mapped have the correct
        // attributes.
        unsafe {
            idmap.map_region(&SHARED_RAM, MT_DEVICE);
            idmap.map_region(&DEVICE0, MT_DEVICE);
            idmap.map_region(&DEVICE1, MT_DEVICE);
        }
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

    fn services() -> &'static [&'static dyn rf_a_bl31::services::Service] {
        &PLATFORM_SERVICES
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
        Some(QemuPsciPlatformImpl::new())
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

    /// Dumps relevant GIC and CCI registers.
    ///
    /// Clobbers x0-x11, x16, x17, sp.
    #[unsafe(naked)]
    unsafe extern "C" fn dump_registers() {
        naked_asm!(
            asm_macros_common!(),
            gic_debug_macros!(),
            "mov_imm x16, {GICD_BASE}",
            "arm_print_gic_regs",
            "ret",
            gic_debug_macros_purge!(),
            asm_macros_common_purge!(),
            DEBUG = const DEBUG as i32,
            ICC_SRE_SRE_BIT = const IccSreEl3::SRE.bits(),
            GICD_BASE = const GICD_BASE,
            GICD_ISPENDR = const offset_of!(Gicd, ispendr),
        );
    }
}
