// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Service for Arm architecture SMCs.

use core::marker::PhantomData;

use crate::{
    context::World,
    errata_framework::{CveId, PlatformErrata, cve_known},
    services::{Service, owns},
    smccc::{
        FunctionId, INVALID_PARAMETER, NOT_REQUIRED, NOT_SUPPORTED, OwningEntityNumber, SUCCESS,
        SetFrom, SmcReturn, SmcccCallType,
    },
};
use zerocopy::IntoBytes;

pub(crate) const SMCCC_VERSION: u32 = 0x8000_0000;
const SMCCC_ARCH_FEATURES: u32 = 0x8000_0001;
const SMCCC_ARCH_SOC_ID_32: u32 = 0x8000_0002;
const SMCCC_ARCH_SOC_ID_64: u32 = 0xc000_0002;
const SMCCC_ARCH_SOC_ID_VERSION: u32 = 0x0;
const SMCCC_ARCH_SOC_ID_REVISION: u32 = 0x1;
const SMCCC_ARCH_SOC_ID_NAME: u32 = 0x2;
const SMCCC_ARCH_WORKAROUND_1: u32 = 0x8000_8000;
const SMCCC_ARCH_WORKAROUND_2: u32 = 0x8000_7FFF;
const SMCCC_ARCH_WORKAROUND_3: u32 = 0x8000_3FFF;
const SMCCC_ARCH_WORKAROUND_4: u32 = 0x8000_0004;

const WORKAROUND_1_CVE: CveId = CveId::new(2017, 5715); // aka Spectre v2
const WORKAROUND_2_CVE: CveId = CveId::new(2018, 3639); // aka Spectre v4
const WORKAROUND_3_CVE: CveId = CveId::new(2022, 23960); // aka Spectre BHB
const WORKAROUND_4_CVE: CveId = CveId::new(2024, 7881);

pub(crate) const SMCCC_VERSION_1_5: i32 = 0x0001_0005;

/// SoC version, including JEDEC JEP-106 Manufacturer ID Code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocVersion {
    bank_index: u8,
    id_code: u8,
    soc_id: u16,
}

impl SocVersion {
    const BANK_INDEX_MASK: u32 = 0x7f;
    const BANK_INDEX_SHIFT: u32 = 24;
    const ID_CODE_SHIFT: u32 = 16;
    const SOC_ID_SHIFT: u32 = 0;

    /// Creates new instance from JEP106 code and implementation defined SoC ID.
    pub const fn new(bank_index: u8, id_code: u8, soc_id: u16) -> Self {
        assert!(bank_index as u32 & !Self::BANK_INDEX_MASK == 0);

        Self {
            bank_index,
            id_code,
            soc_id,
        }
    }
}

impl From<SocVersion> for u32 {
    fn from(value: SocVersion) -> Self {
        (u32::from(value.bank_index) << SocVersion::BANK_INDEX_SHIFT)
            | (u32::from(value.id_code) << SocVersion::ID_CODE_SHIFT)
            | (u32::from(value.soc_id) << SocVersion::SOC_ID_SHIFT)
    }
}

/// Platform dependent Arch service interface.
pub trait ArchPlatform: Sync {
    /// Returns the SiP defined SoC version. The SoC version and revision must uniquely identify the
    /// SoC.
    fn soc_version(&self) -> SocVersion;

    /// Returns the SiP defined SoC revision in bits \[30:0\].  The SoC version and revision must
    /// uniquely identify the SoC.
    fn soc_revision(&self) -> u32;

    /// Optionally returns the SiP-defined SoC name. The maximum length is 135 bytes. The SoC name
    /// must not contain SoC identifying information not captured by <SoC version, SoC revision>.
    fn soc_name(&self) -> Option<&'static str> {
        None
    }
}

/// Arm architecture SMCs.
pub struct Arch<ArchPlatformImpl: ArchPlatform, ErrataPlatformImpl: PlatformErrata> {
    arch_platform: ArchPlatformImpl,
    _errata_phantom: PhantomData<ErrataPlatformImpl>,
}

impl<ArchPlatformImpl: ArchPlatform, ErrataPlatformImpl: PlatformErrata> Service
    for Arch<ArchPlatformImpl, ErrataPlatformImpl>
{
    owns!(OwningEntityNumber::ARM_ARCHITECTURE);

    fn handle_smc(&self, regs: &mut SmcReturn, world: World) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            SMCCC_VERSION => regs.set_from(Self::SMCCC_VERSION),
            SMCCC_ARCH_FEATURES => {
                let res = Self::arch_features(in_regs[1] as u32);
                regs.set_from(res);
            }
            SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => {
                self.arch_soc_id(regs, function.call_type());
            }
            SMCCC_ARCH_WORKAROUND_1
            | SMCCC_ARCH_WORKAROUND_2
            | SMCCC_ARCH_WORKAROUND_3
            | SMCCC_ARCH_WORKAROUND_4 => {
                if Self::arch_features(function.0) >= 0 {
                    // Currently none of the workarounds require any extra actions here. This does
                    // not mean that these calls are no-op: workarounds are present at EL3 entry.
                    // Callers can check implementation status using the SMCCC_ARCH_FEATURES call.
                    regs.mark_empty();
                } else {
                    regs.set_from(NOT_SUPPORTED);
                }
            }
            _ => regs.set_from(NOT_SUPPORTED),
        }

        world
    }
}

impl<ArchPlatformImpl: ArchPlatform, ErrataPlatformImpl: PlatformErrata>
    Arch<ArchPlatformImpl, ErrataPlatformImpl>
{
    const SMCCC_VERSION: i32 = SMCCC_VERSION_1_5;

    /// Creates a new instance of the `Arch` service.
    pub const fn new(arch_platform: ArchPlatformImpl) -> Self {
        Self {
            arch_platform,
            _errata_phantom: PhantomData,
        }
    }

    fn arch_features(arch_func_id: u32) -> i32 {
        match arch_func_id {
            SMCCC_VERSION | SMCCC_ARCH_FEATURES | SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => {
                SUCCESS
            }
            SMCCC_ARCH_WORKAROUND_1 => {
                // As the SMCCC version 1.7, sections 7.7.3-7.7.4 state: CVE-2022-23960 extends
                // CVE-2017-5715 – it is natural for a workaround that mitigates CVE-2022-23960 to
                // also mitigate CVE-2017-5715. Any SMCCC_ARCH_WORKAROUND_3 implementation mitigates
                // both.
                const {
                    if cve_known(ErrataPlatformImpl::ERRATA_LIST, WORKAROUND_1_CVE)
                        || cve_known(ErrataPlatformImpl::ERRATA_LIST, WORKAROUND_3_CVE)
                    {
                        // According to the SMCCC version 1.7, section 7.5.2, a nonnegative value should
                        // be returned if: SMCCC_ARCH_WORKAROUND_1 can be invoked safely on all PEs in
                        // the system.
                        0
                    } else {
                        NOT_SUPPORTED
                    }
                }
            }
            SMCCC_ARCH_WORKAROUND_2 => {
                const {
                    if cve_known(ErrataPlatformImpl::ERRATA_LIST, WORKAROUND_2_CVE) {
                        // According to the SMCCC version 1.7, section 7.6.2, `NOT_REQUIRED` should be
                        // returned if: For all PEs in the system, firmware mitigation for CVE-2018-3639
                        // is either permanently enabled or not required.
                        NOT_REQUIRED
                    } else {
                        NOT_SUPPORTED
                    }
                }
            }
            SMCCC_ARCH_WORKAROUND_3 => {
                const {
                    if cve_known(ErrataPlatformImpl::ERRATA_LIST, WORKAROUND_3_CVE) {
                        // According to the SMCCC version 1.7, section 7.7.2, a nonnegative value should
                        // be returned if: SMCCC_ARCH_WORKAROUND_3 can be invoked safely on all PEs in
                        // the system.
                        0
                    } else {
                        NOT_SUPPORTED
                    }
                }
            }
            SMCCC_ARCH_WORKAROUND_4 => {
                const {
                    if cve_known(ErrataPlatformImpl::ERRATA_LIST, WORKAROUND_4_CVE) {
                        // According to the SMCCC version 1.7, section 7.9.2, 0 should be returned if:
                        // SMCCC_ARCH_WORKAROUND_4 is implemented on the calling PE, and CVE-2024-7881
                        // is mitigated by a higher EL.
                        0
                    } else {
                        NOT_SUPPORTED
                    }
                }
            }
            _ => NOT_SUPPORTED,
        }
    }

    /// This SMC is specified in §7.4 of [the Arm SMC Calling
    /// Convention](https://developer.arm.com/documentation/den0028/galp1/?lang=en).
    fn arch_soc_id(&self, regs: &mut SmcReturn, call_type: SmcccCallType) {
        let soc_id_type = regs.values()[1] as u32;

        match soc_id_type {
            SMCCC_ARCH_SOC_ID_VERSION => regs.set_from(u32::from(self.arch_platform.soc_version())),
            SMCCC_ARCH_SOC_ID_REVISION => regs.set_from(self.arch_platform.soc_revision()),
            SMCCC_ARCH_SOC_ID_NAME if call_type == SmcccCallType::Fast64 => {
                if let Some(name) = self.arch_platform.soc_name() {
                    let args = regs.mark_all_used();
                    args[0] = SUCCESS as u64;

                    let bytes = &mut args[1..].as_mut_bytes();
                    assert!(name.len() < bytes.len());

                    bytes[0..name.len()].copy_from_slice(name.as_bytes());
                    bytes[name.len()..].fill(0);
                } else {
                    regs.set_from(INVALID_PARAMETER)
                }
            }
            _ => regs.set_from(INVALID_PARAMETER),
        }
    }
}
