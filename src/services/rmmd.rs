// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::{cell::RefCell, slice::from_raw_parts_mut};
use num_enum::TryFromPrimitive;
use percore::{Cores, ExceptionLock, PerCore};
use spin::Once;

use crate::{
    context::{CoresImpl, PerCoreState, World},
    info,
    platform::{Platform, PlatformImpl, exception_free},
    services::{Service, owns},
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
};

const RMM_BOOT_VERSION: u64 = 0x5;
/// Size in bytes of the EL3 - RMM shared area.
pub const RMM_SHARED_BUFFER_SIZE: usize = 0x1000;

pub mod manifest;
/// Returns a mutable reference to the shared buffer used for communication between R-EL2 and EL3.
///
/// # Safety
///
/// Calling this function is always safe, but using its return value is safe if all the conditions
/// below are met:
///
/// - It can only be called after the shared buffer is mapped into the page table.
/// - After calling `get_shared_buffer`, the return reference must be dropped before any other call
///   to it is made.
/// - The reference must be dropped before switching to Realm World.
unsafe fn get_shared_buffer() -> &'static mut [u8; RMM_SHARED_BUFFER_SIZE] {
    // Safety: (relative to [`slice::from_raw_parts_mut`][https://doc.rust-lang.org/stable/core/slice/fn.from_raw_parts_mut.html])
    // - The first condition of `get_shared_buffer()` ensures that the location is valid, and as it
    //   occupies exactly one page, it will always be aligned.
    // - `u8` is properly initialized regardless of the initial value.
    // - The second condition ensures that the buffer is never accessed through multiple reference
    //   within EL3. As it can only be accessed by EL3 and Realm World, it follows from the third
    //   condition that no other pointers can be used to access the buffer while a reference exists.
    // - Follows from the soundness of the layout defined in `layout.rs`.
    unsafe {
        from_raw_parts_mut(
            PlatformImpl::RMM_SHARED_BUFFER_START as *mut u8,
            RMM_SHARED_BUFFER_SIZE,
        )
        .try_into()
        .unwrap()
    }
}

const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;
const RMM_RMI_REQ_COMPLETE: u32 = 0xC400_018F;

#[derive(Debug)]
struct RmmdLocal {
    activation_token: Option<u64>,
}

impl RmmdLocal {
    const fn new() -> Self {
        Self {
            activation_token: None,
        }
    }
}

pub static RMM_COLD_BOOT_DONE: Once<()> = Once::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u32)]
enum RmiFuncId {
    DataCreate = 0xC400_0153,
    DataCreateUnknown = 0xC400_0154,
    DataDestroy = 0xC400_0155,
    Features = 0xC400_0165,
    GranuleDelegate = 0xC400_0151,
    GranuleUndelegate = 0xC400_0152,
    PsciComplete = 0xC400_0164,
    RealmActivate = 0xC400_0157,
    RealmCreate = 0xC400_0158,
    RealmDestroy = 0xC400_0159,
    RecAuxCount = 0xC400_0167,
    RecCreate = 0xC400_015A,
    RecDestroy = 0xC400_015B,
    RecEnter = 0xC400_015C,
    RttCreate = 0xC400_015D,
    RttDestroy = 0xC400_015E,
    RttFold = 0xC400_0166,
    RttInitRipas = 0xC400_0168,
    RttMapUnprotected = 0xC400_015F,
    RttReadEntry = 0xC400_0161,
    RttSetRipas = 0xC400_0169,
    RttUnmapUnprotected = 0xC400_0162,
    Version = 0xC400_0150,
}

/// Arm CCA SMCs, for communication between RF-A and TF-RMM.
///
/// This is described at
/// <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html>
pub struct Rmmd {
    core_local: PerCoreState<RmmdLocal>,
}

impl Service for Rmmd {
    owns! {OwningEntityNumber::STANDARD_SECURE, 0x0150..=0x01CF}

    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        if RmiFuncId::try_from(regs.values()[0] as u32).is_ok() {
            World::Realm
        } else {
            regs.set_from(NOT_SUPPORTED);
            World::NonSecure
        }
    }

    fn handle_realm_smc(&self, regs: &mut SmcReturn) -> World {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        match function.0 {
            RMM_BOOT_COMPLETE => {
                info!("Realm boot completed with code 0x{:x}", regs.values()[1]);
                self.handle_boot_complete(regs)
            }

            RMM_RMI_REQ_COMPLETE => {
                // Only x1-x6 are used for RMI return values, the remaining ones MBZ.
                regs.values_mut().copy_within(1..7, 0);
                regs.values_mut()[6..].fill(0);

                World::NonSecure
            }
            _ => {
                regs.set_from(NOT_SUPPORTED);
                World::Realm
            }
        }
    }
}

impl Rmmd {
    pub(super) fn new() -> Self {
        let core_local = PerCore::new(
            [const { ExceptionLock::new(RefCell::new(RmmdLocal::new())) };
                PlatformImpl::CORE_COUNT],
        );

        // Safety:
        // - This function is called after initializing the MMU and pagetable.
        // - This function never calls again `get_shared_buffer()`, thus the reference will be dropped
        //   upon return, before another call is made.
        // - Similarly to the above, this function does not switch to the Realm World.
        let buf = unsafe { get_shared_buffer() };
        PlatformImpl::rme_prepare_manifest(buf);
        info!("RMM Boot Manifest ready");

        Self { core_local }
    }

    /// Initializes the set of registers to pass to R-EL2 after waking up from a suspend.
    ///
    /// <https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html#warm-boot-interface>
    pub(crate) fn handle_wake_from_cpu_suspend(&self) -> [u64; 4] {
        let activation_token = exception_free(|token| {
            self.core_local
                .get()
                .borrow(token)
                .borrow()
                .activation_token
                .unwrap_or_default()
        });

        [CoresImpl::core_index() as u64, activation_token, 0, 0]
    }

    fn handle_boot_complete(&self, regs: &mut SmcReturn) -> World {
        let ret = regs.values()[1] as i32;

        if ret != 0 {
            panic!("RMM Boot failed (code: {ret})")
        }

        exception_free(|token| {
            let mut state = self.core_local.get().borrow_mut(token);

            if state.activation_token.is_none() {
                let activation_token = regs.values()[2];
                info!("Received activation token {activation_token:#x?}");
                state.activation_token = Some(activation_token)
            }
        });

        RMM_COLD_BOOT_DONE.call_once(|| ());
        regs.set_from(ret);

        World::NonSecure
    }

    pub fn entrypoint_args(&self) -> [u64; 8] {
        let core_linear_id = CoresImpl::core_index() as u64;
        if RMM_COLD_BOOT_DONE.is_completed() {
            // When warmbooting a PE for the first time, it should only receive the core id as
            // per the RMM-EL3 warmboot interface. Activation token is set to 0 as it was not
            // generated for this core yet. Subsequent warmboot parameters on this PE will be
            // provided by [`Rmmd::handle_wake_from_cpu_suspend`].
            //
            // https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html#warm-boot-interface
            [core_linear_id, 0, 0, 0, 0, 0, 0, 0]
        } else {
            [
                core_linear_id,
                RMM_BOOT_VERSION,
                PlatformImpl::CORE_COUNT as u64,
                PlatformImpl::RMM_SHARED_BUFFER_START as u64,
                0,
                0,
                0,
                0,
            ]
        }
    }
}
