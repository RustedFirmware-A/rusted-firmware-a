// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{World, cpu_state, world_context, world_cpu_context},
    platform::exception_free,
    smccc::SmcReturn,
};
use arm_sysregs::{
    Esr, ExceptionLevel, Gcscr, HcrEl2, ScrEl3, SctlrEl1, SctlrEl2, Spsr, StackPointer,
    read_gcscr_el1, read_gcscr_el2, read_hcr_el2, read_id_aa64dfr1_el1, read_id_aa64mmfr1_el1,
    read_id_aa64pfr1_el1, read_sctlr_el1, read_sctlr_el2, read_vbar_el1, read_vbar_el2,
    write_elr_el1, write_elr_el2, write_esr_el1, write_esr_el2, write_spsr_el1, write_spsr_el2,
};
#[cfg(not(test))]
use core::arch::asm;
use core::fmt::Debug;
use log::trace;

// Exception vector offsets.
const CURRENT_EL_SP0: usize = 0x0;
const CURRENT_EL_SPX: usize = 0x200;
const LOWER_EL_AARCH64: usize = 0x400;

/// Handler for injecting undefined exception to lower EL caused by the lower EL accessing system
/// registers of which EL3 firmware is unaware.
///
/// This is a safety net to avoid EL3 panics caused by system register access.
pub fn inject_undef64(world: World) {
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        let el3_state = &mut cpu_state[world].el3_state;

        let elr_el3 = el3_state.elr_el3;
        let old_spsr = el3_state.spsr_el3;
        let to_el = target_el(old_spsr.exception_level(), el3_state.scr_el3);

        if old_spsr & Spsr::M_EXECUTION_STATE != Spsr::empty() {
            panic!("Trying to inject undefined exception to lower EL in AArch32 mode")
        }

        let vbar;
        // Write directly to EL1 or EL2 system registers, because we don't save or restore the lower
        // EL system registers in this path.
        match to_el {
            ExceptionLevel::El1 => {
                vbar = read_vbar_el1();
                write_elr_el1(elr_el3);
                write_esr_el1(Esr::IL);
                write_spsr_el1(old_spsr);
            }
            ExceptionLevel::El2 => {
                vbar = read_vbar_el2();
                write_elr_el2(elr_el3);
                write_esr_el2(Esr::IL);
                write_spsr_el2(old_spsr);
            }
            ExceptionLevel::El3 => panic!("Trying to inject undefined exception at EL3"),
            ExceptionLevel::El0 => unreachable!(),
        }

        el3_state.spsr_el3 = create_spsr(old_spsr, to_el);
        el3_state.elr_el3 = find_exception_vector(old_spsr, vbar, to_el);
    });
}

/// Returns the exception level at which an exception should be injected, based on the exception
/// level which caused the original exception.
fn target_el(from_el: ExceptionLevel, scr: ScrEl3) -> ExceptionLevel {
    if from_el > ExceptionLevel::El1 {
        from_el
    } else if is_tge_enabled() && !is_secure_trap_without_sel2(scr) {
        ExceptionLevel::El2
    } else {
        ExceptionLevel::El1
    }
}

/// Calculates the exception vector which should be run at the lower EL.
fn find_exception_vector(spsr_el3: Spsr, vbar: usize, target_el: ExceptionLevel) -> usize {
    let outgoing_el = spsr_el3.exception_level();
    if outgoing_el == target_el {
        if spsr_el3.stack_pointer() == StackPointer::ElX {
            vbar + CURRENT_EL_SPX
        } else {
            vbar + CURRENT_EL_SP0
        }
    } else {
        vbar + LOWER_EL_AARCH64
    }
}

fn is_tge_enabled() -> bool {
    read_id_aa64mmfr1_el1().is_feat_vhe_present() && read_hcr_el2().contains(HcrEl2::TGE)
}

/// Returns whether we are in secure state on a system without S-EL2.
///
/// This can be used to ensure that undef injection does not happen into a non-existent S-EL2. This
/// could happen when a trap happens from S-EL{1,0} and non-secure world is running with TGE bit
/// set, because EL3 does not save/restore EL2 registers if only one world has EL2 enabled. So
/// reading hcr_el2.TGE would give the NS world value.
fn is_secure_trap_without_sel2(scr: ScrEl3) -> bool {
    !scr.contains(ScrEl3::NS) && !scr.contains(ScrEl3::EEL2)
}

/// Explicitly create all bits of SPSR to get PSTATE at exception return.
///
/// The code is based on "Aarch64.exceptions.takeexception" described in DDI0602 revision 2025-09.
/// <https://developer.arm.com/documentation/ddi0602/2025-09/Shared-Pseudocode/aarch64-exceptions-takeexception>
///
/// NOTE: This piece of code must be reviewed every release to ensure that we keep up with new ARCH
/// features which introduces a new SPSR bit.
fn create_spsr(old_spsr: Spsr, target_el: ExceptionLevel) -> Spsr {
    let mut new_spsr = Spsr::empty();
    let sctlr_el1 = read_sctlr_el1();
    let sctlr_el2 = read_sctlr_el2();

    // Set M bits for target EL in AArch64 mode.
    if target_el == ExceptionLevel::El2 {
        new_spsr |= Spsr::M_AARCH64_EL2H;
    } else {
        new_spsr |= Spsr::M_AARCH64_EL1H;
    }

    // Mask all exceptions, update DAIF bits
    new_spsr |= Spsr::D | Spsr::A | Spsr::I | Spsr::F;

    // BTYPE bits are left cleared to ensure that when injecting an undefined exception,
    // BTI does not trigger when performing an exception return as it will be unexpected.

    // If FEAT_SSBS is implemented, take the value from SCTLR.DSSBS
    if read_id_aa64pfr1_el1().is_feat_ssbs_present()
        && ((target_el == ExceptionLevel::El1 && sctlr_el1.contains(SctlrEl1::DSSBS))
            || (target_el == ExceptionLevel::El2 && sctlr_el2.contains(SctlrEl2::DSSBS)))
    {
        new_spsr |= Spsr::SSBS;
    }

    // If FEAT_NMI is implemented, ALLINT = !(SCTLR.SPINTMASK)
    if read_id_aa64pfr1_el1().is_feat_nmi_present()
        && ((target_el == ExceptionLevel::El1 && !sctlr_el1.contains(SctlrEl1::SPINTMASK))
            || (target_el == ExceptionLevel::El2 && !sctlr_el2.contains(SctlrEl2::SPINTMASK)))
    {
        new_spsr |= Spsr::ALLINT;
    }

    // Update PSTATE.PAN bit
    // NOTE: We assume that FEAT_PAN is present as it is mandatory from Armv8.1.
    if old_spsr.contains(Spsr::PAN)
        || (target_el == ExceptionLevel::El1 && !sctlr_el1.contains(SctlrEl1::SPAN))
        || (target_el == ExceptionLevel::El2
            && is_tge_enabled()
            && !sctlr_el2.contains(SctlrEl2::SPAN))
    {
        new_spsr |= Spsr::PAN;
    }

    // DIT bits are unchanged
    new_spsr |= old_spsr & Spsr::DIT;

    // If FEAT_MTE is implemented, mask tag faults by setting TCO bit
    if read_id_aa64pfr1_el1().is_feat_mte_present() {
        new_spsr |= Spsr::TCO;
    }

    // NZCV bits are unchanged
    new_spsr |= old_spsr & Spsr::NZCV;

    // If FEAT_EBEP is implemented, set PM bit
    if read_id_aa64dfr1_el1().is_feat_ebep_present() {
        new_spsr |= Spsr::PM;
    }

    // If FEAT_GCS is implemented, update EXLOCK bit
    if read_id_aa64pfr1_el1().is_feat_gcs_present() {
        let gcscr = if target_el == ExceptionLevel::El2 {
            read_gcscr_el2()
        } else {
            read_gcscr_el1()
        };
        if gcscr.contains(Gcscr::EXLOCKEN) {
            new_spsr |= Spsr::EXLOCK;
        }
    }

    // The IL, SS, UAO, PPEND, PACM and UINJ bits are left cleared as AArch64.TakeException() clears
    // the corresponding PSTATE bits.

    new_spsr
}

/// Describes the reason why execution returned to EL3 after running a lower EL.
#[derive(Debug)]
pub enum RunResult {
    /// A lower EL has executed an SMC instruction.
    Smc,
    /// An IRQ or FIQ routed to EL3 has been triggered while running in a lower EL.
    Interrupt,
    /// A lower EL tried to access a system register that was trapped to EL3.
    SysregTrap { esr: Esr },
}

impl RunResult {
    pub const SMC: u64 = 0;
    pub const INTERRUPT: u64 = 1;
    pub const SYSREG_TRAP: u64 = 2;
}

/// Enters a lower EL in the specified world.
///
/// Exit EL3 and enter a lower EL by ERET. The caller must ensure that if necessary, the contents of
/// the lower EL's system registers have already been restored (i.e. by calling
/// [`crate::context::switch_world()`]). If the contents of one or more GP registers are specified
/// in the `in_regs` parameter, those values will be copied into the lower EL's saved context before
/// the ERET. After execution returns to EL3 by any exception, the reason for returning is checked
/// and the appropriate result will be returned by this function.
pub fn enter_world(regs: &mut SmcReturn, world: World) -> RunResult {
    trace!("Entering world {world:?} with args {regs:x?}");

    if !regs.is_empty() {
        exception_free(|token| {
            cpu_state(token)[world].gpregs.write_return_value(regs);
        });
    }

    let context = world_cpu_context(world);
    let per_world_context = world_context(world);
    let out_values = regs.mark_all_used();
    let return_reason: u64;
    let esr: u64;

    // SAFETY: The CPU context is always valid, and will only be used via this pointer by assembly
    // code after the Rust code returns to prepare for the eret, and after the next exception before
    // entering the Rust code again.
    #[cfg(not(test))]
    unsafe {
        asm!(
            // Save x19 and x29 manually as Rust won't let us specify them as clobbers.
            "stp x19, x29, [sp, #-16]!",
            "bl el3_exit",
            "ldp x19, x29, [sp], #16",
            inout("x0") context => out_values[0],
            inout("x1") per_world_context => out_values[1],
            out("x2") out_values[2],
            out("x3") out_values[3],
            out("x4") out_values[4],
            out("x5") out_values[5],
            out("x6") out_values[6],
            out("x7") out_values[7],
            out("x8") out_values[8],
            out("x9") out_values[9],
            out("x10") out_values[10],
            out("x11") out_values[11],
            out("x12") out_values[12],
            out("x13") out_values[13],
            out("x14") out_values[14],
            out("x15") out_values[15],
            out("x16") out_values[16],
            out("x17") out_values[17],
            out("x18") return_reason,
            out("x20") esr,
            out("x21") _,
            out("x22") _,
            out("x23") _,
            out("x24") _,
            out("x25") _,
            out("x26") _,
            out("x27") _,
            out("x28") _,
            out("x30") _,
        );
    }
    #[cfg(test)]
    {
        let _ = context;
        let _ = per_world_context;
        out_values[0] = 42;
        return_reason = RunResult::SMC;
        esr = 0;
    }

    let result = match return_reason {
        RunResult::SMC => RunResult::Smc,
        RunResult::INTERRUPT => RunResult::Interrupt,
        RunResult::SYSREG_TRAP => RunResult::SysregTrap {
            esr: Esr::from_bits_retain(esr),
        },
        r => panic!("unhandled enter world result: {r}"),
    };

    trace!("Returned from world {world:?} with result {result:?}");

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_result_debug_format() {
        assert_eq!(format!("{:?}", RunResult::Smc), "Smc");
        assert_eq!(format!("{:?}", RunResult::Interrupt), "Interrupt");
        assert_eq!(
            format!(
                "{:?}",
                RunResult::SysregTrap {
                    esr: Esr::from_bits_retain(0x12345)
                }
            ),
            "SysregTrap { esr: Esr(0x12345) }"
        );
    }
}
