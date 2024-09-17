// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Platform;
use crate::{
    context::EntryPointInfo,
    gicv3, logger,
    pagetable::{map_region, IdMap, MT_DEVICE},
    semihosting::{semihosting_exit, AdpStopped},
    services::arch::WorkaroundSupport,
    sysregs::SpsrEl3,
};
use aarch64_paging::paging::MemoryRegion;
use arm_gic::{
    gicv3::{GicV3, SecureIntGroup},
    {IntId, Trigger},
};
use arm_pl011_uart::{OwnedMmioPointer, PL011Registers, Uart};
use core::ptr::NonNull;
use gicv3::{GicConfig, SecureInterruptConfig};
use log::LevelFilter;
use percore::Cores;

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

/// Base address of the primary PL011 UART.
const PL011_BASE_ADDRESS: *mut PL011Registers = 0x0900_0000 as _;
/// Base address of GICv3 distributor.
const GICD_BASE_ADDRESS: *mut u64 = 0x800_0000 as _;
/// Base address of the first GICv3 redistributor frame.
const GICR_BASE_ADDRESS: *mut u64 = 0x80A_0000 as _;
/// Size of a single GIC redistributor frame (there is one per core).
// TODO: Maybe GIC should infer the frame size based on info gicv3 vs gicv4.
// Because I think only 1 << 0x11 and 1 << 0x12 values are allowed.
const GICR_FRAME_SIZE: usize = 1 << 0x11;

// TODO: Use the correct addresses here.
/// The physical address of the SPMC manifest blob.
const TOS_FW_CONFIG_ADDRESS: u64 = 0;
const HW_CONFIG_ADDRESS: u64 = 0;

/// The aarch64 'virt' machine of the QEMU emulator.
pub struct Qemu;

impl Platform for Qemu {
    const CORE_COUNT: usize = 4;

    type LoggerWriter = Uart<'static>;

    const GIC_CONFIG: GicConfig = GicConfig {
        // TODO: Fill this with proper values.
        secure_interrupts_config: &[SecureInterruptConfig {
            id: IntId::spi(0),
            priority: 0x81,
            group: SecureIntGroup::Group1S,
            trigger: Trigger::Level,
        }],
    };

    fn init_beforemmu() {
        // SAFETY: `PL011_BASE_ADDRESS` is the base address of a PL011 device, and nothing else
        // accesses that address range. The address remains valid after turning on the MMU
        // because of the identity mapping of the `DEVICE1` region.
        let uart_pointer =
            unsafe { OwnedMmioPointer::new(NonNull::new(PL011_BASE_ADDRESS).unwrap()) };
        logger::init(Uart::new(uart_pointer), LevelFilter::Trace)
            .expect("Failed to initialise logger");
    }

    fn map_extra_regions(idmap: &mut IdMap) {
        map_region(idmap, &SHARED_RAM, MT_DEVICE);
        map_region(idmap, &DEVICE0, MT_DEVICE);
        map_region(idmap, &DEVICE1, MT_DEVICE);
    }

    unsafe fn create_gic() -> GicV3 {
        // SAFETY: `GICD_BASE_ADDRESS` and `GICR_BASE_ADDRESS` are base addresses of a GIC device,
        // and nothing else accesses that address range.
        // TODO: Powering on-off secondary cores will also access their GIC Redistributors.
        unsafe {
            GicV3::new(
                GICD_BASE_ADDRESS,
                GICR_BASE_ADDRESS,
                Qemu::CORE_COUNT,
                GICR_FRAME_SIZE,
            )
        }
    }

    fn secure_entry_point() -> EntryPointInfo {
        let core_linear_id = Self::core_index() as u64;
        EntryPointInfo {
            pc: 0x0e10_0000,
            #[cfg(feature = "sel2")]
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL2H,
            #[cfg(not(feature = "sel2"))]
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL1H,
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
            spsr: SpsrEl3::D | SpsrEl3::A | SpsrEl3::I | SpsrEl3::F | SpsrEl3::M_AARCH64_EL2H,
            args: Default::default(),
        }
    }

    fn system_off() -> ! {
        semihosting_exit(AdpStopped::ApplicationExit, 0);
        panic!("Semihosting system off call unexpectedly returned.");
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

// SAFETY: This implementation never returns the same index for different cores.
unsafe impl Cores for Qemu {
    fn core_index() -> usize {
        // TODO: Implement this properly. Ensure that the safety invariant still holds, and update
        // the comment to explain how.
        0
    }
}
