// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! FEAT_SCTLR2 extension.
//!
//! This introduces the SCTLR2_ELx registers, which provide top-level control of the system,
//! including its memory system. These registers are extensions of the corresponding SCTLR_ELx
//! registers. FEAT_SCTLR2 is optional from Armv8.0 and mandatory from Armv8.9.

#[cfg(not(feature = "sel2"))]
mod sctlr2_sel1;
#[cfg(feature = "sel2")]
mod sctlr2_sel2;

#[cfg(not(feature = "sel2"))]
use self::sctlr2_sel1::Sctlr2CpuContext;
#[cfg(feature = "sel2")]
use self::sctlr2_sel2::Sctlr2CpuContext;
use super::CpuExtension;
use crate::{
    context::{CPU_DATA_CONTEXT_NUM, PerCoreState, PerWorld, PerWorldContext, World},
    platform::Platform,
};
use arm_sysregs::{ScrEl3, read_id_aa64mmfr3_el1};
#[cfg(not(any(test, feature = "fakes")))]
pub use asm::init_sctlr2_el3;
use core::cell::RefCell;
use percore::{ExceptionLock, PerCore};

/// Enables access to the SCTLR2_ELx registers at lower ELs, along with context switching of those
/// registers on world switch.
pub struct Sctlr2<const CORE_COUNT: usize, PlatformImpl: Platform> {
    context: PerCoreState<CORE_COUNT, PlatformImpl, PerWorld<Sctlr2CpuContext>>,
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Sctlr2<CORE_COUNT, PlatformImpl> {
    /// Constructs a new instance of the SCTLR2 CPU extension.
    pub const fn new() -> Self {
        Self {
            context: PerCore::new(
                [const {
                    ExceptionLock::new(RefCell::new(PerWorld(
                        [Sctlr2CpuContext::EMPTY; CPU_DATA_CONTEXT_NUM],
                    )))
                }; CORE_COUNT],
            ),
        }
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> Default for Sctlr2<CORE_COUNT, PlatformImpl> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CORE_COUNT: usize, PlatformImpl: Platform> CpuExtension
    for Sctlr2<CORE_COUNT, PlatformImpl>
{
    fn is_present(&self) -> bool {
        read_id_aa64mmfr3_el1().is_feat_sctlr2_present()
    }

    fn configure_per_world(&self, _world: World, context: &mut PerWorldContext) {
        // Enable access to SCTLR2_ELx registers at lower ELs.
        context.scr_el3 |= ScrEl3::SCTLR2EN;
    }

    fn save_context(&self, world: World) {
        if self.is_present() {
            self.save_registers(world);
        }
    }

    fn restore_context(&self, world: World) {
        if self.is_present() {
            self.restore_registers(world);
        }
    }
}

#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
mod asm {
    use crate::naked_asm;
    use arm_sysregs::{IdAa64mmfr3El1, Sctlr2El3};

    /// Initialises the SCTLR2_EL3 register if FEAT_SCTLR2 is present.
    ///
    /// This can be called without a valid stack. Clobbers x0.
    #[unsafe(naked)]
    pub extern "C" fn init_sctlr2_el3() {
        naked_asm!(
            "mrs x0, id_aa64mmfr3_el1",
            "ubfx x0, x0, #{SCTLRX_SHIFT}, #{SCTLRX_WIDTH}",
            "cmp x0, {SCTLR2_IMPLEMENTED}",
            "b.ne not_implemented",
            "mov x0, {SCTLR2_EL3_RESET_VAL}",
            "msr sctlr2_el3, x0",
            "not_implemented:",
            "ret",
            SCTLRX_SHIFT = const IdAa64mmfr3El1::SCTLRX_SHIFT,
            SCTLRX_WIDTH = const IdAa64mmfr3El1::SCTLRX_MASK.count_ones(),
            SCTLR2_IMPLEMENTED = const 0b0001,
            SCTLR2_EL3_RESET_VAL = const Sctlr2El3::empty().bits(),
        );
    }
}
