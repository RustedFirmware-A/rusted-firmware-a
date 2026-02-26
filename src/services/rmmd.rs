// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

pub mod manifest;
pub mod svc;

use arm_gpt::GranuleProtection;
use core::{
    cell::RefCell,
    slice::from_raw_parts_mut,
    sync::atomic::{AtomicU8, Ordering},
};
use log::{debug, error, warn};
use num_enum::TryFromPrimitive;
use percore::{Cores, ExceptionLock, PerCore};
use spin::Once;

use spin::mutex::SpinMutex;

use crate::{
    context::{CoresImpl, PerCoreState, World},
    platform::{Platform, PlatformImpl, exception_free},
    services::{
        Service, owns,
        rmmd::svc::{
            Error, RmmAttestGetPlatTokenResponse, RmmAttestGetRealmKeyResponse, RmmCall,
            RmmCommandReturnCode, RmmEl3FeaturesResponse,
        },
    },
    smccc::{FunctionId, NOT_SUPPORTED, OwningEntityNumber, SetFrom, SmcReturn},
};

const RMM_BOOT_VERSION: u64 = 0x5;
/// Size in bytes of the EL3 - RMM shared area.
pub const RMM_SHARED_BUFFER_SIZE: usize = 0x1000;

/// Returns a mutable reference to the shared buffer used for communication between R-EL2 and EL3.
///
/// # Safety
///
/// Calling this function and using its return value is safe if all the conditions below are met:
///
/// 1. It can only be called after the shared buffer is mapped into the page table.
/// 2. After calling `get_shared_buffer`, the return reference must be dropped before any other call
///    to it is made.
/// 3. No PE is running in Realm World while the reference is held, otherwise see [`get_shared_buffer_slice`].
unsafe fn get_shared_buffer() -> &'static mut [u8; RMM_SHARED_BUFFER_SIZE] {
    // Safety: (relative to [`slice::from_raw_parts_mut`][https://doc.rust-lang.org/stable/core/slice/fn.from_raw_parts_mut.html])
    // - Condition #1 ensures that the location is valid, and as it occupies exactly one page, it
    //   will always be aligned.
    // - `u8` is properly initialized regardless of the initial value.
    // - Condition #2 ensures that the buffer is never accessed (read or written) within EL3. It
    //   follows from condition #3 that Realm World cannot access it either. Since only EL3 and
    //   Realm World can access the shared buffer, this requirement is upheld.
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

/// Returns a mutable reference to a slice of the shared buffer used for communication between R-EL2
/// and EL3.
///
/// # Safety
///
/// Calling this function and using its return value is safe if all the conditions below are met:
///
/// 1. It can only be called after the shared buffer is mapped into the page table.
/// 2. The `address` and `len` parameter must be provided by RMM.
/// 3. The returned reference must be dropped before any other call is made with overlapping parameters.
/// 4. The reference must be dropped before switching to Realm World.
unsafe fn get_shared_buffer_slice(
    address: usize,
    len: usize,
) -> Result<&'static mut [u8], RmmCommandReturnCode> {
    check_shared_buffer_range(address, len)?;
    // Safety: (relative to [`slice::from_raw_parts_mut`][https://doc.rust-lang.org/stable/core/slice/fn.from_raw_parts_mut.html])
    // - Condition #1 of `get_shared_buffer()` ensures that the location is valid, and as it
    //   occupies exactly one page, it will always be aligned.
    // - `u8` is properly initialized regardless of the initial value.
    // - Condition #2 ensures that the buffer is never accessed through multiple reference
    //   by other PEs as RMM is responsible for handling concurrency over the shared buffer. As it
    //   can only be accessed by EL3 and Realm World, it follows from the condition #3 that no
    //   other pointers can be used to access the buffer while a reference exists. The condition #4
    //   ensures that Realm World cannot access the buffer while the reference exists.
    // - Follows from the soundness of the layout defined in `layout.rs`.
    Ok(unsafe { from_raw_parts_mut(address as *mut u8, len) })
}

/// Checks that `buf_pa..buf_size` is a valid subrange of the shared buffer.
fn check_shared_buffer_range(buf_pa: usize, buf_size: usize) -> Result<(), RmmCommandReturnCode> {
    let shared_buffer_range = PlatformImpl::RMM_SHARED_BUFFER_START
        ..PlatformImpl::RMM_SHARED_BUFFER_START + RMM_SHARED_BUFFER_SIZE;

    if !shared_buffer_range.contains(&buf_pa) {
        Err(RmmCommandReturnCode::BadAddress)
    } else if !(buf_pa.checked_add(buf_size).and_then(|v| v.checked_sub(1)))
        .is_some_and(|end| shared_buffer_range.contains(&end))
    {
        Err(RmmCommandReturnCode::InvalidValue)
    } else {
        Ok(())
    }
}

const RMM_BOOT_COMPLETE: u32 = 0xC400_01CF;

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

// TODO: these should instead come from a SSL crate.
/// Size in bytes of a SHA256 sum.
const SHA256_DIGEST_SIZE: usize = 32;
/// Size in bytes of a SHA384 sum.
const SHA384_DIGEST_SIZE: usize = 48;
/// Size in bytes of a SHA512 sum.
const SHA512_DIGEST_SIZE: usize = 64;

/// Valid digest sizes for SHA challenges.
const VALID_HASH_SIZES: &[usize] = &[SHA256_DIGEST_SIZE, SHA384_DIGEST_SIZE, SHA512_DIGEST_SIZE];
/// Maximum value of [`VALID_HASH_SIZES`].
const MAX_HASH_SIZE: usize = SHA512_DIGEST_SIZE;

#[derive(Debug, PartialEq)]
#[repr(u8)]
enum RmmBootState {
    Unknown,
    ColdBootDone,
    Error,
}

pub static GRANULE_PROTECTION_TABLE: Once<SpinMutex<GranuleProtection>> = Once::new();

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
    RmmAuxCount = 0xC400_0167,
    RmmCreate = 0xC400_015A,
    RmmDestroy = 0xC400_015B,
    RmmEnter = 0xC400_015C,
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
    attestation_token_read_index: SpinMutex<usize>,
    // Boot status of RMM across all cores.
    // If RMM fails to boot on any core then it is disabled for all cores.
    rmm_boot_state: AtomicU8,
}

impl Service for Rmmd {
    owns! {OwningEntityNumber::STANDARD_SECURE, 0x0150..=0x01CF}

    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        // Only forward RMI calls, if the RMM successfully booted.
        if RmiFuncId::try_from(regs.values()[0] as u32).is_ok() && self.boot_success() {
            World::Realm
        } else {
            // Unsupported FuncId or RMM not booted yet or RMM boot failure
            regs.set_from(NOT_SUPPORTED);
            World::NonSecure
        }
    }

    fn handle_realm_smc(&self, regs: &mut SmcReturn) -> World {
        if self.boot_failure() {
            regs.set_from(NOT_SUPPORTED);
            return World::NonSecure;
        }
        match self.try_handle_realm_smc(regs) {
            Ok(world) => world,
            Err(rec) => {
                regs.set_from(rec);
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
        // - This function is called before the first switch to Realm world and, similarly to above,
        //   the reference is dropped before that switch.
        let buf = unsafe { get_shared_buffer() };
        PlatformImpl::rme_prepare_manifest(buf);
        debug!("RMM Boot Manifest ready");

        #[cfg(all(target_arch = "aarch64", not(test)))]
        GRANULE_PROTECTION_TABLE.call_once(|| {
            // Safety: this code can only be executed once due to [`Once::call_once`]. The
            // `discover()` function is not called anywhere else in the code.
            let gpt = unsafe { GranuleProtection::discover() }.unwrap();
            debug!("GPT discovered: {gpt:x?}");
            SpinMutex::new(gpt)
        });

        Self {
            core_local,
            attestation_token_read_index: SpinMutex::new(0),
            rmm_boot_state: AtomicU8::new(RmmBootState::Unknown as u8),
        }
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
    pub(crate) fn boot_success(&self) -> bool {
        self.rmm_boot_state.load(Ordering::Acquire) == RmmBootState::ColdBootDone as u8
    }

    pub(crate) fn boot_failure(&self) -> bool {
        self.rmm_boot_state.load(Ordering::Acquire) == RmmBootState::Error as u8
    }

    pub(crate) fn set_boot_success(&self) -> Result<u8, u8> {
        // only set ColdBootDone if state was Unknown. Otherwise a race condition between cores
        // booting the RMM could result in overwriting the Error state.
        self.rmm_boot_state.compare_exchange(
            RmmBootState::Unknown as u8,
            RmmBootState::ColdBootDone as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        )
    }

    pub(crate) fn set_boot_failure(&self) {
        self.rmm_boot_state
            .store(RmmBootState::Error as u8, Ordering::Release);
    }

    /// Attempts to handle a SMC originating from Realm World, returning an appropriate code on
    /// error.
    fn try_handle_realm_smc(&self, regs: &mut SmcReturn) -> Result<World, RmmCommandReturnCode> {
        let in_regs = regs.values();
        let mut function = FunctionId(in_regs[0] as u32);
        function.clear_sve_hint();

        if function.0 == RMM_BOOT_COMPLETE {
            debug!("Realm boot completed with code 0x{:x}", regs.values()[1]);
            return Ok(self.handle_boot_complete(regs));
        }

        let command = match RmmCall::from_regs(regs.values()) {
            Ok(command) => command,
            Err(Error::UnrecognisedFunctionId(_)) => {
                regs.set_from(NOT_SUPPORTED);
                return Ok(World::Realm);
            }
            Err(_) => return Err(RmmCommandReturnCode::InvalidValue),
        };

        match command {
            RmmCall::RmiReqComplete { regs: req_regs } => {
                // Only x1-x6 are used for RMI return values, the remaining ones MBZ.
                regs.values_mut()[..6].copy_from_slice(&req_regs);
                regs.values_mut()[6..].fill(0);

                Ok(World::NonSecure)
            }
            RmmCall::GtsiDelegate { .. } => todo!(),
            RmmCall::GtsiUndelegate { .. } => todo!(),
            // TODO(firme): equivalent to FIRME_ATTEST_RAK_GET, will have to take into account the
            // write offset and continued request.
            RmmCall::AttestGetRealmKey {
                buf_pa,
                buf_size,
                ecc_curve,
            } => {
                // Safety:
                // - This function can only be reached after having setup the Realm World, which
                //   requires the MMU and pagetables to be setup.
                // - All parameters come from the SMC arguments provided by RMM.
                // - This function never calls again `get_shared_buffer()`, thus the reference will
                //   be dropped upon return, before another call is made.
                // - Similarly to the above, this function does not switch to the Realm World.
                let shared_buffer = unsafe { get_shared_buffer_slice(buf_pa, buf_size)? };

                let key_size = PlatformImpl::read_attestation_key(shared_buffer, ecc_curve)?;

                regs.set_from(RmmAttestGetRealmKeyResponse { key_size });
                Ok(World::Realm)
            }
            RmmCall::AttestGetPlatToken {
                buf_pa,
                buf_size,
                c_size,
            } => {
                let mut idx = self.attestation_token_read_index.lock();

                // Challenge size must either be a valid hash size or 0 in the case where we resume
                // reading from an already generated token.
                let is_first_chunk = *idx == 0;
                if (is_first_chunk && !VALID_HASH_SIZES.contains(&c_size))
                    || (!is_first_chunk && c_size != 0)
                {
                    return Err(RmmCommandReturnCode::InvalidValue);
                }

                // Safety:
                // - This function can only be reached after having setup the Realm World, which
                //   requires the MMU and pagetables to be setup.
                // - All parameters come from the SMC arguments provided by RMM.
                // - This function never calls again `get_shared_buffer()`, thus the reference will
                //   be dropped upon return, before another call is made.
                // - Similarly to the above, this function does not switch to the Realm World.
                let shared_buffer =
                    unsafe { get_shared_buffer_slice(buf_pa, buf_size.max(c_size))? };

                // If not generating the first chunck, `c_size` will be zero and the hash will not
                // written nor passed to `PlatformImpl::read_attestation_token`.
                let mut hash = [0u8; MAX_HASH_SIZE];
                hash[..c_size].copy_from_slice(&shared_buffer[..c_size]);

                let (size, rem) = PlatformImpl::read_attestation_token(
                    &mut shared_buffer[..buf_size],
                    &hash[..c_size],
                    *idx,
                )?;

                if rem > 0 {
                    *idx += size;
                } else {
                    *idx = 0;
                }

                regs.set_from(RmmAttestGetPlatTokenResponse {
                    token_hunk_size: size,
                    remaining_size: rem,
                });
                Ok(World::Realm)
            }
            RmmCall::El3Features { .. } => {
                regs.set_from(RmmEl3FeaturesResponse { feat_reg: 0 });
                Ok(World::Realm)
            }
            RmmCall::El3TokenSign { .. } => todo!(),
            // TODO: Hacky trick to avoid TF-RMM from enabling encryption (not implemented yet).
            RmmCall::MecRefresh { .. } => {
                regs.set_from(NOT_SUPPORTED);
                Ok(World::Realm)
            }
            RmmCall::IdeKeyProg { .. } => todo!(),
            RmmCall::IdeKeySetGo { .. } => todo!(),
            RmmCall::IdeKeySetStop { .. } => todo!(),
            RmmCall::IdeKmPullResponse { .. } => todo!(),
            RmmCall::ReserveMemory { .. } => todo!(),
        }
    }

    fn handle_boot_complete(&self, regs: &mut SmcReturn) -> World {
        let ret = regs.values()[1] as i32;

        if ret != 0 {
            error!("RMM boot failed with code {ret}");
            self.set_boot_failure();
            return World::NonSecure;
        }

        exception_free(|token| {
            let mut state = self.core_local.get().borrow_mut(token);

            if state.activation_token.is_none() {
                let activation_token = regs.values()[2];
                debug!("Received activation token {activation_token:#x?}");
                state.activation_token = Some(activation_token);

                // set_boot_success() can fail if RmmBootState is already at the Error state.
                // This can happen if another core just failed booting the RMM.
                // Not setting RmmBootState to success is the correct thing to do in this case,
                // therefore the Result is ignored.
                let _ = self.set_boot_success();
                regs.set_from(ret);

                World::NonSecure
            } else {
                warn!(
                    "Received multiple `RMM_BOOT_COMPLETE` SMCs from core {}",
                    CoresImpl::core_index()
                );

                regs.set_from(NOT_SUPPORTED);
                World::Realm
            }
        })
    }

    pub fn entrypoint_args(&self) -> [u64; 8] {
        let core_linear_id = CoresImpl::core_index() as u64;
        if self.boot_success() {
            // When warmbooting a PE for the first time, it should only receive the core id as
            // per the RMM-EL3 warmboot interface. Activation token is set to 0 as it was not
            // generated for this core yet. Subsequent warmboot parameters on this PE will be
            // provided by [`Rmmd::handle_wake_from_cpu_suspend`].
            //
            // https://trustedfirmware-a.readthedocs.io/en/latest/components/rmm-el3-comms-spec.html#warm-boot-interface
            [core_linear_id, 0, 0, 0, 0, 0, 0, 0]
        } else {
            // In case of an unsuccessful RMM boot, we return the coldboot args.
            // This choice is arbitrary, as no more RMM boots will be attempted using these
            // args.
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
#[cfg(test)]
mod tests {
    use crate::{
        context::World,
        services::{
            Rmmd, Service,
            rmmd::{RMM_BOOT_COMPLETE, RmiFuncId, RmmBootState},
        },
        smccc::{SetFrom, SmcReturn},
    };

    fn setup() -> Rmmd {
        Rmmd::new()
    }

    #[test]
    fn boot_complete_test() {
        let rmmd = setup();
        let mut regs = SmcReturn::EMPTY;
        regs.set_from(RMM_BOOT_COMPLETE);
        regs.mark_all_used();
        let world = rmmd.handle_realm_smc(&mut regs);
        assert_eq!(world, World::NonSecure);
        assert!(rmmd.boot_success());
    }

    #[test]
    fn boot_complete_failure_test() {
        let rmmd = setup();
        let mut regs = SmcReturn::EMPTY;
        // Test failing RMM boot
        regs.set_args2(RMM_BOOT_COMPLETE.into(), u64::MAX);
        regs.mark_all_used();
        let world = rmmd.handle_realm_smc(&mut regs);
        assert_eq!(world, World::NonSecure);
        assert_eq!(rmmd.boot_failure(), true);
        // Test returning -1 on RMI calls after failed boot
        regs.mark_empty();
        // Pass any existing RmiFuncId, to make sure the return value is because of the boot failure
        regs.set_from(RmiFuncId::Version as u64);
        let world = rmmd.handle_non_secure_smc(&mut regs);
        assert_eq!(world, World::NonSecure);
        assert_eq!(regs.values()[0], u64::MAX);
    }
}
