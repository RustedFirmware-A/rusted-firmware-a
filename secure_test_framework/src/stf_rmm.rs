// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake RMM component of RF-A Secure Test Framework.

#![no_main]
#![no_std]

extern crate alloc;

mod exceptions;
mod ffa;
mod framework;
mod gicv3;
mod heap;
mod logger;
mod pagetable;
mod platform;
mod secondary;
mod tests;
mod util;

use core::{arch::naked_asm, panic::PanicInfo, ptr::read};

use aarch64_paging::paging::PAGE_SIZE;
use aarch64_rt::{enable_mmu, entry, set_exception_vector};
use log::{error, info};
use smccc::{psci, smc64};

use crate::{
    platform::{Platform, PlatformImpl, RMM_IDMAP},
    secondary::secondary_entry,
    util::current_el,
};

const SUPPORTED_RMM_VERSION: RmmBootManifestVersion = RmmBootManifestVersion { major: 0, minor: 8 };
const SUPPORTED_RMM_MANIFEST_VERSION: RmmBootManifestVersion =
    RmmBootManifestVersion { major: 0, minor: 5 };

const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;
const RMM_RMI_REQ_COMPLETE: u32 = 0xC400_018F;
const RMM_RMI_REQ_VERSION: u64 = 0xC400_0150;

enable_mmu!(RMM_IDMAP);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RmmBootManifestVersion {
    pub(crate) major: u16,
    pub(crate) minor: u16,
}

impl TryFrom<u32> for RmmBootManifestVersion {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let major = (value >> 16) as u16;

        if major & 0x7fff != major {
            return Err(());
        }

        Ok(Self {
            major,
            minor: (value & 0xFFFF) as u16,
        })
    }
}

impl From<RmmBootManifestVersion> for u32 {
    fn from(value: RmmBootManifestVersion) -> Self {
        (value.major as u32) << 16 | value.minor as u32
    }
}
impl From<RmmBootManifestVersion> for u64 {
    fn from(value: RmmBootManifestVersion) -> Self {
        u32::from(value) as u64
    }
}

/// Value returned by RMM after a cold/warmboot.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum RmmBootReturn {
    /// Boot successful.
    Success = 0,
    /// Unknown error.
    Unknown = -1,
    /// Boot Interface version reported by EL3 is not supported by RMM.
    VersionNotValid = -2,
    /// Number of CPUs reported by EL3 larger than maximum supported by RMM.
    CpusOutOfRange = -3,
    /// Current CPU Id is higher or equal than the number of CPUs supported by RMM.
    CpuIdOutOfRange = -4,
    /// Invalid pointer to shared memory area.
    InvalidSharedBuffer = -5,
    /// Version reported by the Boot Manifest not supported by RMM.
    ManifestVersionNotSupported = -6,
    /// Error parsing core Boot Manifest.
    ManifestDataError = -7,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
enum RmiStatusCode {
    Success = 0,
    ErrorInput = 1,
    ErrorRealm = 2,
    #[allow(unused)]
    ErrorRec = 3,
    ErrorRtt = 4,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct RmiCommandReturnCode {
    status: RmiStatusCode,
    index: Option<u8>,
}

impl RmiCommandReturnCode {
    const STATUS_SHIFT: u64 = 0;
    const STATUS_MASK: u64 = 0xFF;

    const INDEX_SHIFT: u64 = 8;
    const INDEX_MASK: u64 = 0xFF;

    fn new(status: RmiStatusCode, index: Option<u8>) -> Self {
        assert!(
            index.is_none()
                || status == RmiStatusCode::ErrorRealm
                || status == RmiStatusCode::ErrorRtt
        );

        Self { status, index }
    }
}

impl From<RmiCommandReturnCode> for u64 {
    fn from(value: RmiCommandReturnCode) -> Self {
        (((value.status as u64) & RmiCommandReturnCode::STATUS_MASK)
            << RmiCommandReturnCode::STATUS_SHIFT)
            | (((value.index.unwrap_or_default() as u64) & RmiCommandReturnCode::INDEX_MASK)
                << RmiCommandReturnCode::INDEX_SHIFT)
    }
}

/// Returns to EL3 with the `RMM_BOOT_COMPLETE` SMC and the specified return code and arguments.
///
/// If `ret` is [`RmmBootReturn::Success`] this function may eventually return with the registers
/// of a function call from EL3 into RMM. If `ret` is anything other than
/// [`RmmBootReturn::Success`], this never returns.
#[inline(always)]
fn complete_boot(ret: RmmBootReturn, extra_args: &[u64]) -> [u64; 18] {
    assert!(extra_args.len() <= 16);

    // Sends a RMM_BOOT_COMPLETE SMC to notify the Root World that RMM has booted.
    let mut args: [u64; 17] = [0; 17];
    args[1..1 + extra_args.len()].copy_from_slice(extra_args);

    args[0] = ret as u64;
    let regs = smc64(RMM_BOOT_COMPLETE, args);

    if ret != RmmBootReturn::Success {
        panic!("RMM called after failed boot")
    }

    regs
}

/// Infinite loop to handle RMI calls coming from NS World.
fn handle_incoming_calls(mut regs: [u64; 18]) -> ! {
    info!("Received RMI call for FID 0x{:X}", regs[0]);

    loop {
        let ret = handle_rmi_call(&regs);

        regs = smc64(RMM_RMI_REQ_COMPLETE, ret);
    }
}

/// Handles a single RMI call from NS world.
fn handle_rmi_call(regs: &[u64]) -> [u64; 17] {
    match regs[0] {
        RMM_RMI_REQ_VERSION => rmi_version(&regs[1..]),
        _ => {
            let mut ret = [0; 17];
            ret[0] = u64::MAX;
            ret
        }
    }
}

fn rmi_version(args: &[u64]) -> [u64; 17] {
    let mut ret = [0; 17];

    let Ok(requested) = RmmBootManifestVersion::try_from(args[0] as u32) else {
        ret[0] = RmiCommandReturnCode::new(RmiStatusCode::ErrorInput, None).into();
        return ret;
    };

    info!(
        "Received RMM_RMI_REQ_VERSION for v{}.{}",
        requested.major, requested.minor
    );

    ret[0] = if requested == SUPPORTED_RMM_VERSION {
        RmiCommandReturnCode::new(RmiStatusCode::Success, None).into()
    } else {
        RmiCommandReturnCode::new(RmiStatusCode::ErrorInput, None).into()
    };
    ret[1] = SUPPORTED_RMM_VERSION.into();
    ret[2] = SUPPORTED_RMM_VERSION.into();

    ret
}

fn validate_coldboot_args(pe_idx: u64, version: u64, core_count: u64, shared_buffer_addr: u64) {
    if pe_idx >= core_count {
        complete_boot(RmmBootReturn::CpuIdOutOfRange, &[]);
    }

    let Ok(version) = RmmBootManifestVersion::try_from(version as u32) else {
        complete_boot(RmmBootReturn::VersionNotValid, &[]);
        unreachable!()
    };

    if version.major != SUPPORTED_RMM_VERSION.major {
        complete_boot(RmmBootReturn::VersionNotValid, &[]);
        unreachable!()
    }

    if shared_buffer_addr == 0 {
        complete_boot(RmmBootReturn::InvalidSharedBuffer, &[]);
        unreachable!()
    }

    // Shared buffer must be paged-aligned.
    if !shared_buffer_addr.is_multiple_of(PAGE_SIZE as u64) {
        complete_boot(RmmBootReturn::InvalidSharedBuffer, &[]);
        unreachable!()
    }

    // TODO: map the shared buffer into memory.

    if core_count > PlatformImpl::CORE_COUNT as u64 {
        complete_boot(RmmBootReturn::CpusOutOfRange, &[]);
        unreachable!()
    }
}

/// Indicates whether the image already went through a cold boot.
///
/// This variable must only be accessed by [`entrypoint`] while MMU and caches are disabled.
static mut NEEDS_COLD_BOOT: bool = true;

#[unsafe(naked)]
#[unsafe(link_section = ".init.entrypoint")]
#[unsafe(export_name = "entrypoint")]
unsafe extern "C" fn entrypoint() -> ! {
    naked_asm!(
        // Fetches NEEDS_COLD_BOOT.
        "adrp x30, {marker}",
        "add x30, x30, :lo12:{marker}",
        "ldrb w29, [x30]",
        "cmp w29, #0",
        "b.eq 1f",

        // If NEEDS_COLD_BOOT == true, set it to false and go to aarch64_rt::entry.
        "mov w29, #0",
        "strb w29, [x30]",
        "b entry",

        // Otherwise jump to [`crate::secondary::secondary_entry`].
        "1:",
        "b {warmboot_main}",
        marker = sym NEEDS_COLD_BOOT,
        warmboot_main = sym secondary_entry,
    )
}

entry!(rmm_main, 4);

fn rmm_main(pe_idx: u64, version: u64, core_count: u64, shared_buffer_addr: u64) -> ! {
    validate_coldboot_args(pe_idx, version, core_count, shared_buffer_addr);

    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink).unwrap();

    info!(
        "Fake RMM starting at EL {} with args {:#x}, {:#x}, {:#x}, {:#x}",
        current_el(),
        pe_idx,
        version,
        core_count,
        shared_buffer_addr,
    );

    // Safety: the specification states that the `x3` register of the RMM is a pointer to a 4KB
    // page mapped into the Realm World. Since this is coldboot, no other PE is running hence no
    // concurrent write can occur.
    let manifest_version = unsafe { read(shared_buffer_addr as *mut u32) };

    let Ok(manifest_version) = RmmBootManifestVersion::try_from(manifest_version) else {
        complete_boot(RmmBootReturn::VersionNotValid, &[]);
        unreachable!()
    };

    info!(
        "Received manifest with version v{}.{}",
        manifest_version.major, manifest_version.minor
    );

    if manifest_version.major != SUPPORTED_RMM_MANIFEST_VERSION.major {
        error!("Unsupported manifest version: 0x{manifest_version:x?}");
        complete_boot(RmmBootReturn::ManifestVersionNotSupported, &[]);
        unreachable!()
    }

    let regs = complete_boot(RmmBootReturn::Success, &[0xdead_beef]);
    handle_incoming_calls(regs)
}

fn secondary_main(pe_idx: u64, activation_token: u64) -> ! {
    set_exception_vector();

    info!(
        "Fake RMM warmboot at EL {} with args {:#x}, {:#x}",
        current_el(),
        pe_idx,
        activation_token,
    );

    let generated_activation_token = if activation_token == 0 {
        0xdead_beef
    } else {
        0
    };

    let regs = complete_boot(RmmBootReturn::Success, &[generated_activation_token]);
    handle_incoming_calls(regs)
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    error!("{info}");
    loop {}
}

fn call_test_helper(_: usize, _: [u64; 3]) -> Result<[u64; 4], ()> {
    panic!("call_test_helper shouldn't be called from realm world tests");
}

/// Not supported in RMM.
pub fn start_secondary(psci_mpidr: u64, _entry: fn(u64) -> !, arg: u64) -> Result<(), psci::Error> {
    panic!("start_secondary({psci_mpidr:#}, .., {arg}) called in RMM");
}
