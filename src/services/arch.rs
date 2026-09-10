// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Service for Arm architecture SMCs.

use crate::{
    context::World,
    services::{Service, owns},
    smccc::{
        FunctionId, INVALID_PARAMETER, NOT_SUPPORTED, OwningEntityNumber, SUCCESS, SetFrom,
        SmcReturn, SmcccCallType,
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

/// Whether a particular arch workaround SMC is required or not.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum WorkaroundSupport {
    /// The workaround call is required.
    Required = 0,
    /// The workaround is safe to call but not required.
    SafeButNotRequired = 1,
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

    /// Returns whether this platform supports the arch WORKAROUND_1 SMC.
    fn arch_workaround_1_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_1 SMC.
    fn arch_workaround_1() {}

    /// Returns whether this platform supports the arch WORKAROUND_2 SMC.
    fn arch_workaround_2_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_2 SMC.
    fn arch_workaround_2() {}

    /// Returns whether this platform supports the arch WORKAROUND_3 SMC.
    fn arch_workaround_3_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }

    /// If safe and necessary, performs the workaround specified for the WORKAROUND_3 SMC.
    fn arch_workaround_3() {}

    /// Returns whether this platform supports the arch WORKAROUND_4 SMC.
    fn arch_workaround_4_supported() -> WorkaroundSupport {
        WorkaroundSupport::SafeButNotRequired
    }
}

/// Arm architecture SMCs.
pub struct Arch<ArchPlatformImpl: ArchPlatform> {
    platform: ArchPlatformImpl,
}

impl<ArchPlatformImpl: ArchPlatform> Service for Arch<ArchPlatformImpl> {
    owns!(OwningEntityNumber::ARM_ARCHITECTURE);

    fn handle_smc(&self, regs: &mut SmcReturn, world: World) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            SMCCC_VERSION => regs.set_from(Self::SMCCC_VERSION),
            SMCCC_ARCH_FEATURES => Self::arch_features(regs),
            SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => {
                self.arch_soc_id(regs, function.call_type());
            }
            SMCCC_ARCH_WORKAROUND_1 => {
                Self::arch_workaround_1();
                regs.mark_empty();
            }
            SMCCC_ARCH_WORKAROUND_2 => {
                Self::arch_workaround_2(in_regs[1] as u32);
                regs.mark_empty();
            }
            SMCCC_ARCH_WORKAROUND_3 => {
                Self::arch_workaround_3();
                regs.mark_empty();
            }
            _ => regs.set_from(NOT_SUPPORTED),
        }

        world
    }
}

impl<ArchPlatformImpl: ArchPlatform> Arch<ArchPlatformImpl> {
    const SMCCC_VERSION: i32 = SMCCC_VERSION_1_5;

    /// Creates a new instance of the `Arch` service.
    pub const fn new(platform: ArchPlatformImpl) -> Self {
        Self { platform }
    }

    fn arch_features(regs: &mut SmcReturn) {
        let arch_func_id = regs.values()[1] as u32;

        let result = match arch_func_id {
            SMCCC_VERSION | SMCCC_ARCH_FEATURES | SMCCC_ARCH_SOC_ID_32 | SMCCC_ARCH_SOC_ID_64 => {
                SUCCESS
            }
            SMCCC_ARCH_WORKAROUND_1 => ArchPlatformImpl::arch_workaround_1_supported() as i32,
            SMCCC_ARCH_WORKAROUND_2 => ArchPlatformImpl::arch_workaround_2_supported() as i32,
            SMCCC_ARCH_WORKAROUND_3 => ArchPlatformImpl::arch_workaround_3_supported() as i32,
            SMCCC_ARCH_WORKAROUND_4 => ArchPlatformImpl::arch_workaround_4_supported() as i32,
            _ => NOT_SUPPORTED,
        };

        regs.set_from(result);
    }

    /// This SMC is specified in §7.4 of [the Arm SMC Calling
    /// Convention](https://developer.arm.com/documentation/den0028/galp1/?lang=en).
    fn arch_soc_id(&self, regs: &mut SmcReturn, call_type: SmcccCallType) {
        let soc_id_type = regs.values()[1] as u32;

        match soc_id_type {
            SMCCC_ARCH_SOC_ID_VERSION => regs.set_from(u32::from(self.platform.soc_version())),
            SMCCC_ARCH_SOC_ID_REVISION => regs.set_from(self.platform.soc_revision()),
            SMCCC_ARCH_SOC_ID_NAME if call_type == SmcccCallType::Fast64 => {
                if let Some(name) = self.platform.soc_name() {
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

    /// Execute the mitigation for CVE-2017-5715 on the calling PE.
    fn arch_workaround_1() {
        if ArchPlatformImpl::arch_workaround_1_supported() == WorkaroundSupport::Required {
            ArchPlatformImpl::arch_workaround_1()
        }
    }

    /// Enable the mitigation for CVE-2018-3639 on the calling PE. (Contrary to the
    /// latest specification as of January 2025, the argument is ignored.)
    fn arch_workaround_2(_: u32) {
        if ArchPlatformImpl::arch_workaround_2_supported() == WorkaroundSupport::Required {
            ArchPlatformImpl::arch_workaround_2()
        }
    }

    /// Execute the mitigation for CVE-2017-5715 and CVE-2022-23960 on the calling PE.
    fn arch_workaround_3() {
        if ArchPlatformImpl::arch_workaround_3_supported() == WorkaroundSupport::Required {
            ArchPlatformImpl::arch_workaround_3()
        }
    }
}
