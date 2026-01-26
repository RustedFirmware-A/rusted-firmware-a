// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake RMM component of RF-A Secure Test Framework.

#![no_main]
#![no_std]
#![allow(dead_code)]

extern crate alloc;

mod exceptions;
mod ffa;
mod framework;
mod gicv3;
mod heap;
mod logger;
mod pagetable;
mod platform;
mod tests;
mod util;

use core::{
    panic::PanicInfo,
    slice::from_raw_parts_mut,
    sync::atomic::{AtomicBool, Ordering},
};

use aarch64_rt::{enable_mmu, entry};
use log::{error, info};
use smccc::{psci, smc64};

use crate::{
    platform::{Platform, PlatformImpl, RMM_IDMAP},
    util::current_el,
};

const SUPPORTED_RMM_VERSION: RmmBootManifestVersion = RmmBootManifestVersion { major: 0, minor: 8 };
const SUPPORTED_RMM_MANIFEST_VERSION: RmmBootManifestVersion =
    RmmBootManifestVersion { major: 0, minor: 5 };

const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;

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

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum RmmBootReturn {
    Success = 0,
    Unknown = -1,
    VersionNotValid = -2,
    CpusOutOfRange = -3,
    CpuIdOutOfRange = -4,
    InvalidSharedBuffer = -5,
    ManifestVersionNotSupported = -6,
    ManifestDataError = -7,
}

fn complete_boot(ret: RmmBootReturn, extra_args: &[u64]) -> ! {
    assert!(extra_args.len() <= 16);

    // Sends a RMM_BOOT_COMPLETE SMC to notify the Root World that RMM has booted.
    let mut args: [u64; 17] = [0; 17];
    args[1..1 + extra_args.len()].copy_from_slice(extra_args);

    args[0] = ret as u64;
    smc64(RMM_BOOT_COMPLETE, args);

    // TODO: handle RMI calls originating from NS world.
    todo!()
}

fn validate_coldboot_args(pe_idx: u64, version: u64, core_count: u64, shared_buffer_addr: u64) {
    if pe_idx >= core_count {
        complete_boot(RmmBootReturn::CpuIdOutOfRange, &[]);
    }

    let Ok(version) = RmmBootManifestVersion::try_from(version as u32) else {
        complete_boot(RmmBootReturn::VersionNotValid, &[])
    };

    if version.major != SUPPORTED_RMM_VERSION.major {
        complete_boot(RmmBootReturn::VersionNotValid, &[])
    }

    if shared_buffer_addr == 0 {
        complete_boot(RmmBootReturn::InvalidSharedBuffer, &[]);
    }

    // Shared buffer must be paged-aligned.
    if !shared_buffer_addr.is_multiple_of(0x1000) {
        complete_boot(RmmBootReturn::InvalidSharedBuffer, &[]);
    }

    // TODO: map the shared buffer into memory.

    if core_count > PlatformImpl::CORE_COUNT as u64 {
        complete_boot(RmmBootReturn::CpusOutOfRange, &[]);
    }
}

/// Indicates whether the image already went through a cold boot.
///
/// TODO: due to a bug likely in [`entry!`], the `.bss` segment is reinitialized each time the
/// machine jumps to the entrypoint, hence overriding any changes done during coldboot. This
/// variable is set to `true` to be stored in the `.data` segment, avoiding the issue.
static NEEDS_COLD_BOOT: AtomicBool = AtomicBool::new(true);

entry!(rmm_main, 4);
fn rmm_main(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    if !NEEDS_COLD_BOOT.load(Ordering::Acquire) {
        warmboot_main(x0, x1)
    } else {
        NEEDS_COLD_BOOT.store(false, Ordering::Release);
        coldboot_main(x0, x1, x2, x3)
    }
}

fn coldboot_main(pe_idx: u64, version: u64, core_count: u64, shared_buffer_addr: u64) -> ! {
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
    // page mapped into the Realm World.
    let shared_buf =
        unsafe { from_raw_parts_mut(shared_buffer_addr as *mut u32, 0x1000 / size_of::<u32>()) };

    let Ok(manifest_version) = RmmBootManifestVersion::try_from(shared_buf[0]) else {
        complete_boot(RmmBootReturn::VersionNotValid, &[]);
    };

    info!(
        "Received manifest with version v{}.{}",
        manifest_version.major, manifest_version.minor
    );

    if manifest_version.major != SUPPORTED_RMM_MANIFEST_VERSION.major {
        error!("Unsupported manifest version: 0x{manifest_version:x?}");
        complete_boot(RmmBootReturn::ManifestVersionNotSupported, &[])
    }

    complete_boot(RmmBootReturn::Success, &[0xdead_beef])
}

fn warmboot_main(pe_idx: u64, activation_token: u64) -> ! {
    let log_sink = PlatformImpl::make_log_sink();
    logger::init(log_sink).unwrap();

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

    complete_boot(RmmBootReturn::Success, &[generated_activation_token])
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
