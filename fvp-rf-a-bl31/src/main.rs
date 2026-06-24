// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! RF-A BL31 deployment for the Arm Fixed Virtual Platform.

#![no_main]
#![no_std]

mod config;
mod psci_platform;
#[cfg(feature = "rme")]
mod rmmd_platform;

#[cfg(feature = "rme")]
use crate::rmmd_platform::FvpRmmdPlatformImpl;
use crate::{
    config::{FVP_CLUSTER_COUNT, FVP_MAX_CPUS_PER_CLUSTER, FVP_MAX_PE_PER_CPU},
    psci_platform::{FvpPsciPlatformImpl, PSCI_STATE_COUNT},
};
use arm_fvp_base_pac::{
    MemoryMap, Peripherals, PhysicalInstance,
    arm_pl011_uart::{Uart, UniqueMmioPointer},
};
#[cfg(feature = "pauth")]
use core::arch::asm;
use core::{
    mem::offset_of,
    ops::{Range, RangeInclusive},
    ptr::NonNull,
};
#[cfg(feature = "pauth")]
use rf_a_bl31::reexports::arm_sysregs::el0::accessors::read_cntpct_el0;
use rf_a_bl31::{
    aarch64::dsb_sy,
    all_asm, asm_macros_common, asm_macros_common_purge, bl31_warm_entrypoint,
    context::{CoresImpl, EntryPointInfo},
    cpu::{aem_generic::AemGeneric, define_cpu_ops},
    cpu_extensions::{
        CpuExtension, amu::Amu, fgt::Fgt, fgt2::Fgt2, fpmr::Fpmr, gcs::Gcs, hcx::Hcx, mpam::Mpam,
        mte2::MemoryTagging, pfar::Pfar, pmuv3::MultiThreadedPmu, ras::Ras, sctlr2::Sctlr2,
        simd::Simd, spe::StatisticalProfiling, sys_reg_trace::SysRegTrace, tcr2::Tcr2,
        trbe::TraceBufferNonSecure, trf::TraceFiltering,
    },
    crash_console::pl011::Pl011CrashConsole,
    debug::DEBUG,
    errata_framework::define_errata_list,
    gic_debug_macros, gic_debug_macros_purge,
    gicv3::{Gic, GicConfig, InterruptConfig},
    logger::LockedWriter,
    naked_asm,
    pagetable::{
        IdMap, MT_DEVICE, MT_MEMORY_EL3,
        early_pagetable::{EarlyRegion, define_early_mapping},
    },
    panic_handler,
    platform::Platform,
    reexports::{
        aarch64_paging::{
            descriptor::VirtualAddress,
            mair::{MairAttribute, NormalMemory},
            paging::MemoryRegion,
        },
        arm_gic::{
            IntId, Trigger,
            gicv3::{Group, HIGHEST_S_PRIORITY, SecureIntGroup, registers::Gicd},
        },
        arm_sysregs::{
            el0::registers::Amcntenset1El0, el1::registers::MpidrEl1, el3::registers::IccSreEl3,
        },
        percore::Cores,
        spin::mutex::SpinMutex,
    },
    services::{
        Service,
        arch::{Arch, ArchPlatform},
        errata_management::ErrataManagement,
        psci::PsciPlatformInterface,
    },
    statics,
};

/// Converts `RangeInclusive` into `Range`.
const fn from_inclusive_range(range_inclusive: &RangeInclusive<usize>) -> Range<usize> {
    *range_inclusive.start()..range_inclusive.end().checked_add(1).unwrap()
}

/// Returns a 2MB aligned `Range` that covers the `start` and `end` ranges.
const fn aligned_range_covering(
    start: &RangeInclusive<usize>,
    end: &RangeInclusive<usize>,
) -> Range<usize> {
    const ALIGN_2MB: usize = 1 << 21;

    assert!(*start.end() < *end.start(), "end must be after start");

    let start_address = *start.start() & !(ALIGN_2MB - 1);
    let end_address = (*end.end() + 1).next_multiple_of(ALIGN_2MB);

    start_address..end_address
}

const UART0_RANGE: Range<usize> = from_inclusive_range(&MemoryMap::UART0);
const UART1_RANGE: Range<usize> = from_inclusive_range(&MemoryMap::UART1);

const CRASH_UART_BASE: usize = *MemoryMap::UART1.start();

/// Peripheral range from VE_SYSTEM to POWER_CONTROLLER.
const DEVICE0_RANGE: Range<usize> =
    aligned_range_covering(&MemoryMap::VE_SYSTEM, &MemoryMap::POWER_CONTROLLER);

/// Peripheral range from REFCLK_CNTCONTROL to AP_REFCLK_CNTBASE1.
const DEVICE1_RANGE: Range<usize> = aligned_range_covering(
    &MemoryMap::REFCLK_CNTCONTROL,
    &MemoryMap::AP_REFCLK_CNTBASE1,
);

/// Peripherals range that covers the GIC.
const DEVICE2_RANGE: Range<usize> = aligned_range_covering(&MemoryMap::GICD, &MemoryMap::GICR);

const PLATFORM_CORE_COUNT: usize =
    FVP_CLUSTER_COUNT * FVP_MAX_CPUS_PER_CLUSTER * FVP_MAX_PE_PER_CPU;

const ARM_TRUSTED_SRAM_RANGE: Range<usize> = from_inclusive_range(&MemoryMap::TRUSTED_SRAM);
const ARM_SHARED_RAM_BASE: usize = ARM_TRUSTED_SRAM_RANGE.start;
const ARM_SHARED_RAM_SIZE: usize = 0x0000_1000; /* 4 KB */

#[cfg(feature = "rme")]
const ARM_GPT_L0_BASE: usize = ARM_TRUSTED_SRAM_RANGE.end - ARM_GPT_L0_SIZE;
#[cfg(feature = "rme")]
const ARM_GPT_L0_SIZE: usize = 0x0000_2000;
#[cfg(feature = "rme")]
const ARM_GPT_L1_BASE: usize = *MemoryMap::DRAM0.end() + 1 - ARM_GPT_L1_SIZE;
#[cfg(feature = "rme")]
const ARM_GPT_L1_SIZE: usize = 0x0010_0000;

const WARM_ENTRYPOINT_FIELD: *mut unsafe extern "C" fn() -> ! = ARM_SHARED_RAM_BASE as _;

const SHARED_RAM: MemoryRegion = MemoryRegion::new(
    ARM_SHARED_RAM_BASE,
    ARM_SHARED_RAM_BASE + ARM_SHARED_RAM_SIZE,
);

// Ideally these regions should be discovered along with the GPT using system-registers
// ([`arm_gpt::GranuleProtection::discover`]) rather than hard-coded.
#[cfg(feature = "rme")]
const GPT_L0: MemoryRegion = MemoryRegion::new(ARM_GPT_L0_BASE, ARM_GPT_L0_BASE + ARM_GPT_L0_SIZE);
#[cfg(feature = "rme")]
const GPT_L1: MemoryRegion = MemoryRegion::new(ARM_GPT_L1_BASE, ARM_GPT_L1_BASE + ARM_GPT_L1_SIZE);

const DEVICE_REGIONS: [MemoryRegion; 3] = [
    MemoryRegion::new(DEVICE0_RANGE.start, DEVICE0_RANGE.end),
    MemoryRegion::new(DEVICE1_RANGE.start, DEVICE1_RANGE.end),
    MemoryRegion::new(DEVICE2_RANGE.start, DEVICE2_RANGE.end),
];

// TODO: These addresses should be parsed from FW_CONFIG
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0x0400_1500;
const NT_FW_CONFIG_ADDRESS: u64 = 0x8000_0000;
const HW_CONFIG_ADDRESS: u64 = 0x07f0_0000;
const HW_CONFIG_ADDRESS_NS: u64 = 0x8200_0000;

const EARLY_REGIONS: [EarlyRegion; 3] = [
    EarlyRegion {
        address_range: ARM_TRUSTED_SRAM_RANGE,
        attributes: MT_MEMORY_EL3,
    },
    EarlyRegion {
        address_range: UART0_RANGE,
        attributes: MT_DEVICE,
    },
    EarlyRegion {
        address_range: UART1_RANGE,
        attributes: MT_DEVICE,
    },
];

define_early_mapping!(Fvp, EARLY_REGIONS);

const fn secure_sgi_configuration(index: u32) -> (IntId, InterruptConfig) {
    (
        IntId::sgi(index),
        InterruptConfig {
            priority: HIGHEST_S_PRIORITY,
            group: Group::Secure(SecureIntGroup::Group1S),
            trigger: Trigger::Edge,
        },
    )
}

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

define_cpu_ops!(Fvp, [AemGeneric]);
define_errata_list!(Fvp, []);

/// Fixed Virtual Platform
struct Fvp;

struct ArchPlatformImpl;

impl ArchPlatform for ArchPlatformImpl {}

static ARCH: Arch<ArchPlatformImpl> = Arch::new();
static ERRATA_MANAGEMENT: ErrataManagement<Fvp> = ErrataManagement::new();

static PLATFORM_SERVICES: [&'static dyn Service; 2] = [&ARCH, &ERRATA_MANAGEMENT];

static AMU: Amu<PLATFORM_CORE_COUNT, CoresImpl<Fvp>> =
    Amu::new([Amcntenset1El0::empty(); PLATFORM_CORE_COUNT], true);

// SAFETY: `core_position` is indeed a naked function, doesn't access the stack or any other memory,
// only clobbers x0-x5, and returns a unique core index as long as `FVP_MAX_CPUS_PER_CLUSTER` and
// `FVP_MAX_PE_PER_CPU` are correct.
unsafe impl Platform for Fvp {
    const CORE_COUNT: usize = PLATFORM_CORE_COUNT;
    const CACHE_WRITEBACK_GRANULE: usize = 1 << 6;

    const PAGE_HEAP_PAGE_COUNT: usize = 6;

    type LogSinkImpl = LockedWriter<Uart<'static>>;
    type IdMap = IdMap<{ Self::PAGE_HEAP_PAGE_COUNT }>;
    type PsciPlatformImpl = FvpPsciPlatformImpl<'static>;
    #[cfg(feature = "rme")]
    type RmmdPlatformImpl = FvpRmmdPlatformImpl;
    type CrashConsoleImpl = Pl011CrashConsole<CRASH_UART_BASE, 24_000_000, 115_200>;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[
            secure_sgi_configuration(8),
            secure_sgi_configuration(9),
            secure_sgi_configuration(10),
            secure_sgi_configuration(11),
            secure_sgi_configuration(12),
            secure_sgi_configuration(13),
            secure_sgi_configuration(14),
            secure_sgi_configuration(15),
        ],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] = &[
        &AMU,
        &Fgt,
        &Fgt2,
        &Fpmr,
        &Gcs,
        &Hcx,
        &MemoryTagging,
        &Mpam,
        &MultiThreadedPmu,
        &Ras,
        &Sctlr2,
        &Simd::simd(),
        &StatisticalProfiling,
        &SysRegTrace,
        &Tcr2,
        &TraceBufferNonSecure,
        &TraceFiltering,
        &Pfar,
    ];

    // Set write-through mode to ensure all written values are propagated to system memory.
    // This guarantees correct Once and Mutex behavior.
    const NORMAL_MEMORY_MAIR_ATTRIBUTE: MairAttribute = MairAttribute::normal(
        NormalMemory::WriteThroughTransientReadWriteAllocate,
        NormalMemory::WriteThroughTransientReadWriteAllocate,
    );

    fn init(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        let peripherals = Peripherals::take().unwrap();

        let uart_pointer = map_peripheral(peripherals.uart0);

        LOGGER
            .init(LockedWriter::new(Uart::new(uart_pointer)))
            .expect("Failed to initialise logger");

        let psci_platform = FvpPsciPlatformImpl::new(
            peripherals.power_controller,
            peripherals.system,
            peripherals.refclk_cntcontrol,
            peripherals.ap_refclk_cntctl,
        );

        psci_platform.init_generic_timer();

        *FVP_PSCI_PLATFORM_IMPL.lock() = Some(psci_platform);

        // Write warm boot entry point the shared memory, so secondary cores can pick it up during
        // boot.
        // Safety: WARM_ENTRYPOINT_FIELD points to a valid, writable address.
        unsafe {
            *WARM_ENTRYPOINT_FIELD = bl31_warm_entrypoint::<Fvp>;
        }
        dsb_sy();

        GIC.call_once(|| {
            let gicd = map_peripheral(peripherals.gicd);
            let mut gicr = map_peripheral(peripherals.gicr);
            // SAFETY: `gicr` points to a continuously mapped GIC redistributor memory area until
            // the last redistributor block. There are no other references to this address range.
            unsafe { Gic::new(gicd, gicr.ptr_nonnull()).unwrap() }
        });
    }

    fn map_extra_regions(idmap: &mut Self::IdMap) {
        // SAFETY: Nothing is being unmapped, and the regions being mapped have the correct
        // attributes.
        unsafe {
            idmap.map_region(&SHARED_RAM, MT_DEVICE);

            #[cfg(feature = "rme")]
            idmap.map_region(&GPT_L0, MT_MEMORY_EL3);
            #[cfg(feature = "rme")]
            idmap.map_region(&GPT_L1, MT_MEMORY_EL3);

            for region in &DEVICE_REGIONS {
                idmap.map_region(region, MT_DEVICE);
            }
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

    fn services() -> &'static [&'static dyn Service] {
        &PLATFORM_SERVICES
    }

    fn handle_group0_interrupt(int_id: IntId) {
        todo!("Handle group0 interrupt {:?}", int_id)
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = CoresImpl::<Self>::core_index() as u64;
        EntryPointInfo {
            pc: 0x0600_0000,
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
            args: [NT_FW_CONFIG_ADDRESS, HW_CONFIG_ADDRESS_NS, 0, 0, 0, 0, 0, 0],
        }
    }

    #[cfg(feature = "rme")]
    fn realm_entry_point() -> EntryPointInfo {
        EntryPointInfo {
            pc: 0xfdc0_0000,
            args: CORE_SERVICES.rmmd.entrypoint_args(),
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

    /// Dumps relevant GIC registers.
    ///
    /// Clobbers x0-x11, x16, x17, sp.
    #[unsafe(naked)]
    unsafe extern "C" fn dump_registers() {
        naked_asm!(
            asm_macros_common!(),
            gic_debug_macros!(),
            "mov_imm	x16, {GICD_BASE}",
            "arm_print_gic_regs",
            "ret",

            gic_debug_macros_purge!(),
            asm_macros_common_purge!(),
            DEBUG = const DEBUG as i32,
            ICC_SRE_SRE_BIT = const IccSreEl3::SRE.bits(),
            GICD_ISPENDR = const offset_of!(Gicd, ispendr),
            GICD_BASE = const *MemoryMap::GICD.start(),
        );
    }
}

all_asm!(Fvp);
statics!(Fvp);
panic_handler!();
