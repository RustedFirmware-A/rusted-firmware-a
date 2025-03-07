// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{CoresImpl, World},
    platform::{exception_free, Platform, PlatformImpl},
    services::{owns, Service},
    smccc::{OwningEntityNumber, SmcReturn},
};
use arm_ffa::{
    DirectMsgArgs, FfaError, Interface, SecondaryEpRegisterAddr, SuccessArgsIdGet,
    SuccessArgsSpmIdGet, TargetInfo, Version,
};
use core::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use log::{debug, error, info, warn};
use percore::{ExceptionLock, PerCore};

const FUNCTION_NUMBER_MIN: u16 = 0x0060;
const FUNCTION_NUMBER_MAX: u16 = 0x00FF;

/// Core local state of the SPMD service
struct SpmdLocal {
    spmc_initialized: bool, // TODO: add enum for SPMC state
    secure_interrupt_in_progress: bool,
}

impl SpmdLocal {
    const fn new() -> Self {
        Self {
            spmc_initialized: false,
            secure_interrupt_in_progress: false,
        }
    }
}

/// Secure Partition Manager Dispatcher, defined by Arm Firmware Framework for A-Profile (FF-A)
pub struct Spmd {
    spmc_id: u16,
    spmc_version: Version,
    spmc_primary_ep: usize,
    spmc_secondary_ep: AtomicUsize,
    core_local: PerCore<ExceptionLock<RefCell<SpmdLocal>>, CoresImpl, { PlatformImpl::CORE_COUNT }>,
}

impl Service for Spmd {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_non_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        // TODO: forward SVE hint bit

        // TODO: should we use a different version for NWd?
        let version = self.spmc_version;

        let (in_regs, mut out_regs) = if version.needs_18_regs() {
            (&regs[..], SmcReturn::from([0u64; 18]))
        } else {
            (&regs[..8], SmcReturn::from([0u64; 8]))
        };

        let in_msg = match Interface::from_regs(version, in_regs) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Invalid FF-A call from Normal World {:#x?}", e);
                Interface::error(e.into()).to_regs(version, out_regs.values_mut());
                return (out_regs, World::NonSecure);
            }
        };

        let (out_msg, next_world) = self.handle_non_secure_call(&in_msg);

        out_msg.to_regs(version, out_regs.values_mut());

        (out_regs, next_world)
    }

    fn handle_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        let version = self.spmc_version;

        let (in_regs, mut out_regs) = if version.needs_18_regs() {
            (&regs[..], SmcReturn::from([0u64; 18]))
        } else {
            (&regs[..8], SmcReturn::from([0u64; 8]))
        };

        let in_msg = match Interface::from_regs(version, in_regs) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Invalid FF-A call from Secure World: {:#x?} ", e);
                Interface::error(e.into()).to_regs(version, out_regs.values_mut());
                return (out_regs, World::Secure);
            }
        };

        let (out_msg, next_world) = self.handle_secure_call(&in_msg);

        // TODO: this is a workaround. Receiving this message from the SPMC means that we have to
        // resume NWd execution after it was preempted by a secure interrupt. Simply forwarding this
        // call to NWd would mean that the function ID gets written to NWd's x0, but in this case we
        // must not modify NWd's state. This can be achieved by returning SmcReturn with the count of
        // used values set to 0. Is there a more ergonomic way to do this?
        if matches!(out_msg, Interface::NormalWorldResume) {
            return (SmcReturn::EMPTY, World::NonSecure);
        }

        out_msg.to_regs(version, out_regs.values_mut());

        (out_regs, next_world)
    }
}

impl Spmd {
    const OWN_ID: u16 = 0xffff;
    const VERSION: Version = Version(1, 2);
    const NS_EP_ID: u16 = 0; // TODO: this should come from arm_ffa

    /// Initialises the SPMD state.
    ///
    /// This should be called exactly once, before any other SPMD methods are called or any
    /// secondary CPUs are started.
    pub(super) fn new() -> Self {
        info!("Initializing SPMD");

        // TODO: read these attributes from the SPMC manifest
        let spmc_id = 0x8000;
        let spmc_version = Version(1, 2);
        let spmc_primary_ep = 0x0600_0000;

        assert!(spmc_version.is_compatible_to(&Spmd::VERSION));

        let core_local = PerCore::new(
            [const { ExceptionLock::new(RefCell::new(SpmdLocal::new())) };
                PlatformImpl::CORE_COUNT],
        );

        Self {
            spmc_id,
            spmc_version,
            spmc_primary_ep,
            // By default the secondary EP is same as primary
            spmc_secondary_ep: spmc_primary_ep.into(),
            core_local,
        }
    }

    pub fn primary_ep(&self) -> usize {
        self.spmc_primary_ep
    }

    pub fn secondary_ep(&self) -> usize {
        self.spmc_secondary_ep.load(Relaxed)
    }

    fn handle_secure_call(&self, in_msg: &Interface) -> (Interface, World) {
        // By default return to the same world
        let mut next_world = World::Secure;

        debug!("Handle FF-A call from SWd {:#x?}", in_msg);

        let out_msg = match in_msg {
            Interface::Error { error_code, .. } => {
                // If we get this at boot, it means the SPMC init failed
                exception_free(|token| {
                    let spmd_state = self.core_local.get().borrow_mut(token);
                    if !spmd_state.spmc_initialized {
                        // TODO: should we return an error instead of panic?
                        panic!("SPMC init failed with error {}", error_code);
                    }
                });

                // Otherwise forward to NWd
                next_world = World::NonSecure;
                *in_msg
            }
            Interface::Version { input_version } => Interface::VersionOut {
                output_version: Self::VERSION.min(*input_version),
            },
            Interface::Features { .. } => {
                // TODO: add list of supported features
                Interface::success32_noargs()
            }
            Interface::IdGet => Interface::Success {
                target_info: TargetInfo::default(),
                args: SuccessArgsIdGet { id: self.spmc_id }.into(),
            },
            Interface::SpmIdGet => Interface::Success {
                target_info: TargetInfo::default(),
                args: SuccessArgsSpmIdGet { id: Self::OWN_ID }.into(),
            },
            Interface::MsgWait { .. } => {
                // Receiving this message for the first time means that SPMC init succeeded
                exception_free(|token| {
                    self.core_local.get().borrow_mut(token).spmc_initialized = true;
                });

                // Forward to NWd
                next_world = World::NonSecure;
                *in_msg
            }
            Interface::NormalWorldResume => {
                exception_free(|token| {
                    let mut spmd_state = self.core_local.get().borrow_mut(token);
                    if spmd_state.secure_interrupt_in_progress {
                        // Interrupt was handled, return to NWd
                        spmd_state.secure_interrupt_in_progress = false;
                        next_world = World::NonSecure;
                        *in_msg
                    } else {
                        // Normal world execution was not preempted
                        Interface::error(FfaError::Denied)
                    }
                })
            }
            Interface::MsgSendDirectResp {
                src_id,
                dst_id,
                args,
            } => {
                if *dst_id == Self::OWN_ID {
                    match *args {
                        DirectMsgArgs::VersionResp { version } if *src_id == self.spmc_id => {
                            next_world = World::NonSecure;
                            match version {
                                None => Interface::error(FfaError::NotSupported),
                                Some(v) => Interface::VersionOut { output_version: v },
                            }
                        }
                        _ => Interface::error(FfaError::InvalidParameters),
                    }
                } else {
                    // Forward to NWd
                    next_world = World::NonSecure;
                    *in_msg
                }
            }
            Interface::SecondaryEpRegister { entrypoint } => {
                // TODO: check if the entrypoint is within the range of the SPMC's memory range
                let secondary_ep = match entrypoint {
                    SecondaryEpRegisterAddr::Addr32(addr) => *addr as usize,
                    SecondaryEpRegisterAddr::Addr64(addr) => *addr as usize,
                };
                self.spmc_secondary_ep.store(secondary_ep, Relaxed);
                Interface::success32_noargs()
            }
            Interface::Success { .. }
            | Interface::Interrupt { .. }
            | Interface::Yield
            | Interface::MemRetrieveResp { .. } => {
                // Forward to NWd
                next_world = World::NonSecure;
                *in_msg
            }
            _ => {
                warn!("Unsupported FF-A call from Secure World: {:#x?}", in_msg);
                Interface::error(FfaError::NotSupported)
            }
        };

        (out_msg, next_world)
    }

    fn handle_non_secure_call(&self, in_msg: &Interface) -> (Interface, World) {
        // By default return to the same world
        let mut next_world = World::NonSecure;

        debug!("Handle FF-A call from NWd {:#x?}", in_msg);

        let out_msg = match in_msg {
            Interface::Version { input_version } => {
                if self.spmc_version == Version(1, 0) {
                    // If the SPMC version is 1.0, we have to show this to NWd
                    Interface::VersionOut {
                        output_version: Version(1, 0),
                    }
                } else {
                    // Forward version call to the SPMC
                    next_world = World::Secure;
                    Interface::MsgSendDirectReq {
                        src_id: Self::OWN_ID,
                        dst_id: self.spmc_id,
                        args: DirectMsgArgs::VersionReq {
                            version: *input_version,
                        },
                    }
                }
            }
            Interface::IdGet => {
                // Return Hypervisor / NWd kernel endpoint ID
                Interface::Success {
                    target_info: TargetInfo::default(),
                    args: SuccessArgsIdGet { id: Self::NS_EP_ID }.into(),
                }
            }
            Interface::SpmIdGet => Interface::Success {
                target_info: TargetInfo::default(),
                args: SuccessArgsSpmIdGet { id: self.spmc_id }.into(),
            },
            Interface::MsgSendDirectReq { src_id, .. } => {
                // Validate source endpoint ID
                // TODO: create a function to check this
                if *src_id & 0x8000 != 0 {
                    Interface::error(FfaError::InvalidParameters)
                } else {
                    // Forward to SWd
                    next_world = World::Secure;
                    *in_msg
                }
            }
            Interface::Error { .. }
            | Interface::Success { .. }
            | Interface::Features { .. }
            | Interface::RxAcquire { .. }
            | Interface::RxRelease { .. }
            | Interface::RxTxMap { .. }
            | Interface::RxTxUnmap { .. }
            | Interface::PartitionInfoGet { .. }
            | Interface::PartitionInfoGetRegs { .. }
            | Interface::Run { .. }
            | Interface::MemDonate { .. }
            | Interface::MemLend { .. }
            | Interface::MemShare { .. }
            | Interface::MemRetrieveReq { .. }
            | Interface::MemReclaim { .. } => {
                // Forward to SWd
                next_world = World::Secure;
                *in_msg
            }
            _ => {
                warn!("Unsupported FF-A call from Normal World: {:#x?}", in_msg);
                Interface::error(FfaError::NotSupported)
            }
        };

        (out_msg, next_world)
    }

    pub fn forward_secure_interrupt(&self) -> (SmcReturn, World) {
        let version = self.spmc_version;

        let mut out_regs = if version.needs_18_regs() {
            SmcReturn::from([0u64; 18])
        } else {
            SmcReturn::from([0u64; 8])
        };

        let msg = Interface::Interrupt {
            // The endpoint and vCPU ID fields MBZ in this case
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0,
            },
            // The SPMD shouldn't query the GIC
            interrupt_id: 0,
        };

        msg.to_regs(version, out_regs.values_mut());

        exception_free(|token| {
            let mut spmd_state = self.core_local.get().borrow_mut(token);
            spmd_state.secure_interrupt_in_progress = true;
        });

        (out_regs, World::Secure)
    }
}
