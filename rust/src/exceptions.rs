// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{cpu_state, World},
    platform::exception_free,
    services::dispatch_smc,
    smccc::FunctionId,
    sysregs::{
        is_feat_vhe_present, read_hcr_el2, read_vbar_el1, read_vbar_el2, write_elr_el1,
        write_elr_el2, write_esr_el1, write_esr_el2, write_spsr_el1, write_spsr_el2, Esr,
        ExceptionLevel, HcrEl2, ScrEl3, Spsr, StackPointer,
    },
};
use log::debug;

const TRAP_RET_UNHANDLED: i64 = -1;

// Exception vector offsets.
const CURRENT_EL_SP0: usize = 0x0;
const CURRENT_EL_SPX: usize = 0x200;
const LOWER_EL_AARCH64: usize = 0x400;

#[unsafe(no_mangle)]
extern "C" fn handle_sysreg_trap(_esr_el3: u64) -> i64 {
    TRAP_RET_UNHANDLED
}

/// Returns the type of the highest priority pending interrupt at the interrupt controller.
#[unsafe(no_mangle)]
extern "C" fn plat_ic_get_pending_interrupt_type() -> u32 {
    unimplemented!();
}

/// Called from the exception handler in assembly to handle an interrupt.
#[unsafe(no_mangle)]
extern "C" fn handle_interrupt(interrupt_type: u32) {
    panic!("Unexpected interrupt of type {}", interrupt_type);
}

/// Handler for injecting undefined exception to lower EL caused by the lower EL accessing system
/// registers of which EL3 firmware is unaware.
///
/// This is a safety net to avoid EL3 panics caused by system register access.
#[unsafe(no_mangle)]
extern "C" fn inject_undef64() {
    exception_free(|token| {
        let mut cpu_state = cpu_state(token);
        let el3_state = &mut cpu_state.context_mut(World::from_scr()).el3_state;

        let elr_el3 = el3_state.elr_el3;
        let old_spsr = el3_state.spsr_el3;
        let to_el = target_el(old_spsr.exception_level(), el3_state.scr_el3);

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
    is_feat_vhe_present() && read_hcr_el2().contains(HcrEl2::TGE)
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
/// The code is based on "Aarch64.exceptions.takeexception" described in DDI0602 revision 2023-06.
/// https://developer.arm.com/documentation/ddi0602/2023-06/Shared-Pseudocode/aarch64-exceptions-takeexception
///
/// NOTE: This piece of code must be reviewed every release to ensure that we keep up with new ARCH
/// features which introduces a new SPSR bit.
fn create_spsr(old_spsr: Spsr, target_el: ExceptionLevel) -> Spsr {
    let mut new_spsr = Spsr::empty();

    // Set M bits for target EL in AArch64 mode.
    if target_el == ExceptionLevel::El2 {
        new_spsr |= Spsr::M_AARCH64_EL2H;
    } else {
        new_spsr |= Spsr::M_AARCH64_EL1H;
    }

    // Mask all exceptions, update DAIF bits
    new_spsr |= Spsr::D | Spsr::A | Spsr::I | Spsr::F;

    // DIT bits are unchanged
    new_spsr |= old_spsr & Spsr::DIT;

    // NZCV bits are unchanged
    new_spsr |= old_spsr & Spsr::NZCV;

    // TODO: Add support for BTI, SSBS, NMI, PAN, UAO, MTE2, EBEP, SEBEP and GCS.

    new_spsr
}

/// Called from the exception handler in assembly to handle an SMC.
#[unsafe(no_mangle)]
extern "C" fn handle_smc(function: FunctionId, x1: u64, x2: u64, x3: u64, x4: u64) {
    let world = World::from_scr();
    debug!(
        "Handling SMC {:?} ({:#0x}, {:#0x}, {:#0x}, {:#0x}) from world {:?}",
        function, x1, x2, x3, x4, world,
    );

    let ret = dispatch_smc(function, x1, x2, x3, x4, world);

    // Write the return value back to the registers of the world that made the SMC call. Note that
    // this might not be the same world as we are about to return to, as the handler might have
    // switched worlds by calling `set_next_world_context`.
    exception_free(|token| {
        cpu_state(token)
            .context_mut(world)
            .gpregs
            .write_return_value(&ret);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::arch::{SMCCC_VERSION, SMCCC_VERSION_1_5},
        sysregs::{fake::SYSREGS, ScrEl3},
    };

    /// Tests the SMCCC arch version call as a simple example of SMC dispatch.
    ///
    /// The point of this isn't to test every individual SMC call, just that the common code in
    /// `handle_smc` works. Individual SMC calls can be tested directly within their modules.
    #[test]
    fn handle_smc_arch_version() {
        // Pretend to be coming from non-secure world.
        SYSREGS.lock().unwrap().scr_el3 = ScrEl3::NS;

        handle_smc(FunctionId(SMCCC_VERSION), 0, 0, 0, 0);

        assert_eq!(
            exception_free(|token| { cpu_state(token).context(World::NonSecure).gpregs.registers }),
            [
                SMCCC_VERSION_1_5 as u64,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ]
        );
    }
}
