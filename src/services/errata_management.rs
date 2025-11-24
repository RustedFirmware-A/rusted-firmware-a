// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{World, cpu_state},
    platform::{ERRATA_LIST, exception_free},
    services::{Service, owns},
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
};
use arm_sysregs::ExceptionLevel;
use log::trace;

const FUNCTION_NUMBER_MIN: u16 = 0x00F0;
const FUNCTION_NUMBER_MAX: u16 = 0x010F;

const EM_VERSION: u32 = 0x8400_00F0;
const EM_FEATURES: u32 = 0x8400_00F1;
const EM_CPU_ERRATUM_FEATURES: u32 = 0x8400_00F2;

const VERSION_1_0: i32 = 0x0001_0000;

/// A status value returned by errata management functions.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
enum Status {
    /// The erratum is fully mitigated at EL3.
    HigherElMitigation = 3,
    /// The erratum has been fixed in hardware.
    NotAffected = 2,
    /// The calling EL is responsible for mitigating the erratum.
    Affected = 1,
    Success = 0,
    NotSupported = -1,
    InvalidParameters = -2,
    /// The erratum either:
    ///   * Isn't known by this build of RF-A.
    ///   * Isn't mitigated at EL3, and can't be mitigated by the calling EL.
    ///   * Is split responsibility and the top half of the workaround isn't implemented by this
    ///     build of RF-A.
    UnknownErratum = -3,
}

impl SetFrom<Status> for SmcReturn {
    fn set_from(&mut self, status: Status) {
        self.set_from(status as i32)
    }
}

/// Arm Errata Management Firmware Interface, as specified by Arm document number DEN0100.
pub struct ErrataManagement;

impl ErrataManagement {
    pub const fn new() -> Self {
        Self
    }
}

impl Service for ErrataManagement {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            EM_VERSION => regs.set_from(version()),
            EM_FEATURES => regs.set_from(features(in_regs[1] as u32)),
            EM_CPU_ERRATUM_FEATURES => regs.set_from(cpu_erratum_features(in_regs)),
            _ => regs.set_from(NOT_SUPPORTED),
        }
        World::NonSecure
    }
}

fn version() -> i32 {
    VERSION_1_0
}

fn features(em_func_id: u32) -> i32 {
    match em_func_id {
        EM_VERSION | EM_FEATURES | EM_CPU_ERRATUM_FEATURES => Status::Success as i32,
        _ => Status::NotSupported as i32,
    }
}

fn cpu_erratum_features(regs: &[u64]) -> Status {
    let cpu_erratum_id = regs[1] as u32;
    let forward_flag = regs[2] != 0;

    if regs[3] != 0 || regs[4] != 0 || regs[5] != 0 || regs[6] != 0 || regs[7] != 0 {
        return Status::InvalidParameters;
    }

    let originator = exception_free(|token| cpu_state(token)[World::NonSecure].el3_state.spsr_el3)
        .exception_level();
    let effective_originator = if forward_flag {
        if originator != ExceptionLevel::El2 {
            return Status::InvalidParameters;
        }
        ExceptionLevel::El1
    } else {
        originator
    };

    trace!("Checking erratum {cpu_erratum_id} for {effective_originator:?}");

    for erratum in ERRATA_LIST {
        if erratum.id == cpu_erratum_id {
            return if (erratum.check)() {
                Status::HigherElMitigation
            } else {
                Status::NotAffected
            };
        }
    }

    Status::UnknownErratum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        errata_framework::Erratum,
        platform::test::{TestMitigatedErratum, TestUnneededErratum},
    };
    use arm_sysregs::SpsrEl3;

    #[test]
    fn em_version_non_secure() {
        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0] = EM_VERSION.into();
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0x0000_0000_0001_0000]);
    }

    #[test]
    fn em_version_secure_not_supported() {
        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0] = EM_VERSION.into();
        assert_eq!(ErrataManagement.handle_secure_smc(&mut regs), World::Secure);
        assert_eq!(regs.values(), [0xffff_ffff_ffff_ffff]);
    }

    #[test]
    fn em_features() {
        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..2].copy_from_slice(&[EM_FEATURES.into(), EM_VERSION.into()]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0]);

        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..2].copy_from_slice(&[EM_FEATURES.into(), EM_FEATURES.into()]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0]);

        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..2]
            .copy_from_slice(&[EM_FEATURES.into(), EM_CPU_ERRATUM_FEATURES.into()]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0]);

        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..2].copy_from_slice(&[EM_FEATURES.into(), 0x8400_00F3]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0xffff_ffff_ffff_ffff]);
    }

    /// Reserved parameters must be 0.
    #[test]
    fn em_cpu_erratum_features_extra_parameters() {
        let mut regs = SmcReturn::EMPTY;
        // x3 is reserved, shouldn't be set.
        regs.mark_all_used()[0..4].copy_from_slice(&[EM_CPU_ERRATUM_FEATURES.into(), 42, 0, 66]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0xffff_ffff_ffff_fffe]);
    }

    /// Calls from NS-EL1 shouldn't pass a non-zero forward_flag.
    #[test]
    fn em_cpu_erratum_features_invalid_forward_flag_nsel1() {
        // Make it look like the non-secure world was in EL1.
        exception_free(|token| {
            cpu_state(token)[World::NonSecure].el3_state.spsr_el3 = SpsrEl3::M_AARCH64_EL1H
        });

        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..3].copy_from_slice(&[EM_CPU_ERRATUM_FEATURES.into(), 42, 1]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0xffff_ffff_ffff_fffe]);

        // Reset the CPU state ready for the next test.
        exception_free(|token| {
            cpu_state(token)[World::NonSecure].el3_state.spsr_el3 = SpsrEl3::empty();
        });
    }

    /// Calls from NS-EL2 can pass a non-zero forward_flag.
    #[test]
    fn em_cpu_erratum_features_valid_forward_flag_nsel2() {
        // Make it look like the non-secure world was in EL2.
        exception_free(|token| {
            cpu_state(token)[World::NonSecure].el3_state.spsr_el3 = SpsrEl3::M_AARCH64_EL2H
        });

        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..3].copy_from_slice(&[EM_CPU_ERRATUM_FEATURES.into(), 42, 1]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0xffff_ffff_ffff_fffd]);

        // Reset the CPU state ready for the next test.
        exception_free(|token| {
            cpu_state(token)[World::NonSecure].el3_state.spsr_el3 = SpsrEl3::empty();
        });
    }

    #[test]
    fn em_cpu_erratum_features_unknown() {
        let mut regs = SmcReturn::EMPTY;
        // Assuming there's no erratum with ID 42.
        regs.mark_all_used()[0..3].copy_from_slice(&[EM_CPU_ERRATUM_FEATURES.into(), 42, 0]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [0xffff_ffff_ffff_fffd]);
    }

    #[test]
    fn em_cpu_erratum_features_mitigated() {
        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..3].copy_from_slice(&[
            EM_CPU_ERRATUM_FEATURES.into(),
            TestMitigatedErratum::ID.into(),
            0,
        ]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [3])
    }

    #[test]
    fn em_cpu_erratum_features_not_needed() {
        let mut regs = SmcReturn::EMPTY;
        regs.mark_all_used()[0..3].copy_from_slice(&[
            EM_CPU_ERRATUM_FEATURES.into(),
            TestUnneededErratum::ID.into(),
            0,
        ]);
        assert_eq!(
            ErrataManagement.handle_non_secure_smc(&mut regs),
            World::NonSecure
        );
        assert_eq!(regs.values(), [2]);
    }
}
