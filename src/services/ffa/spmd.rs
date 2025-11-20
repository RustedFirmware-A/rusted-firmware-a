// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::{PerCoreState, World, switch_world},
    exceptions::{RunResult, enter_world},
    platform::{Platform, PlatformImpl, exception_free},
    services::{Service, owns, psci::PsciSpmInterface},
    smccc::{OwningEntityNumber, SmcReturn},
};
use arm_ffa::{
    DirectMsgArgs, FfaError, Interface, SecondaryEpRegisterAddr, SuccessArgsIdGet,
    SuccessArgsSpmIdGet, TargetInfo, Version, VersionOut, WarmBootType,
};
use core::{
    cell::RefCell,
    sync::atomic::{AtomicUsize, Ordering::Relaxed},
};
use log::{debug, error, info, warn};
use percore::{ExceptionLock, PerCore};

const FUNCTION_NUMBER_MIN: u16 = 0x0060;
const FUNCTION_NUMBER_MAX: u16 = 0x00EF;

/// Core-local state of the SPMD service
struct SpmdLocal {
    spmc_state: SpmcState,
}

impl SpmdLocal {
    const fn new() -> Self {
        Self {
            spmc_state: SpmcState::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpmcState {
    Off,
    Boot,
    Runtime,
    SecureInterrupt,
    PsciEventHandling,
}

/// Secure Partition Manager Dispatcher, defined by Arm Firmware Framework for A-Profile (FF-A)
pub struct Spmd {
    spmc_id: u16,
    spmc_version: Version,
    spmc_primary_ep: usize,
    spmc_secondary_ep: AtomicUsize,
    core_local: PerCoreState<SpmdLocal>,
}

impl Service for Spmd {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        FUNCTION_NUMBER_MIN..=FUNCTION_NUMBER_MAX
    );

    fn handle_non_secure_smc(&self, regs: &mut SmcReturn) -> World {
        // TODO: forward SVE hint bit

        // TODO: should we use a different version for NWd?
        let version = self.spmc_version;

        match &mut Interface::from_regs(version, regs.values()) {
            Ok(msg) => {
                debug!("Handle FF-A call from NWd {msg:x?}");

                let spmc_state =
                    exception_free(|token| self.core_local.get().borrow(token).borrow().spmc_state);

                assert_eq!(spmc_state, SpmcState::Runtime);

                let next_world = self.handle_non_secure_call(msg);

                msg.to_regs(version, regs.mark_all_used());

                next_world
            }
            Err(error) => {
                error!("Invalid FF-A call from Normal World {error}");
                let response = match error {
                    arm_ffa::Error::InvalidVersion(_) => Interface::VersionOut {
                        output_version: VersionOut::NotSupported,
                    },
                    error => Interface::error((*error).into()),
                };

                response.to_regs(version, regs.mark_all_used());
                World::NonSecure
            }
        }
    }

    fn handle_secure_smc(&self, regs: &mut SmcReturn) -> World {
        let version = self.spmc_version;

        match &mut Interface::from_regs(version, regs.values()) {
            Ok(msg) => {
                debug!("Handle FF-A call from SWd {msg:x?}");

                let spmc_state =
                    exception_free(|token| self.core_local.get().borrow(token).borrow().spmc_state);

                let (has_msg, next_world) = match spmc_state {
                    SpmcState::Off => panic!(),
                    SpmcState::Boot => self.handle_secure_call_boot(msg),
                    SpmcState::Runtime => self.handle_secure_call_runtime(msg),
                    SpmcState::SecureInterrupt => self.handle_secure_call_interrupt(msg),
                    SpmcState::PsciEventHandling => self.handle_secure_call_psci_event(msg),
                };

                if has_msg {
                    msg.to_regs(version, regs.mark_all_used());
                } else {
                    regs.mark_empty();
                }

                next_world
            }
            Err(error) => {
                error!("Invalid FF-A call from Secure World: {error} ");
                Interface::error((*error).into()).to_regs(version, regs.mark_all_used());
                World::Secure
            }
        }
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
    pub fn new() -> Self {
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

        let spmd = Self {
            spmc_id,
            spmc_version,
            spmc_primary_ep,
            // By default the secondary EP is same as primary
            spmc_secondary_ep: spmc_primary_ep.into(),
            core_local,
        };

        // This only runs once, on the primary core, at cold boot. Set the correct state before
        // receiving the first message from SWd.
        spmd.switch_spmc_local_state(SpmcState::Off, SpmcState::Boot);

        spmd
    }

    #[allow(unused)]
    pub fn primary_ep(&self) -> usize {
        self.spmc_primary_ep
    }

    pub fn secondary_ep(&self) -> usize {
        self.spmc_secondary_ep.load(Relaxed)
    }

    fn switch_spmc_local_state(&self, expected_state: SpmcState, new_state: SpmcState) {
        exception_free(|token| {
            let spmc_state = &mut self.core_local.get().borrow_mut(token).spmc_state;
            assert_eq!(*spmc_state, expected_state);
            *spmc_state = new_state;
        });
    }

    /// Handles calls originating from the secure world that are handled the same way in all `SpmcState`.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_secure_call_common(&self, msg: &mut Interface) -> (bool, World) {
        *msg = match msg {
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
            _ => {
                warn!("Unsupported FF-A call from Secure World: {msg:x?}");
                Interface::error(FfaError::NotSupported)
            }
        };

        (true, World::Secure)
    }

    /// Handles calls originating from the secure world during the boot phase.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_secure_call_boot(&self, msg: &mut Interface) -> (bool, World) {
        match msg {
            Interface::Error { error_code, .. } => {
                // TODO: should we return an error instead of panic?
                panic!("SPMC init failed with error {error_code}");
            }
            Interface::Version { input_version } => {
                *msg = Interface::VersionOut {
                    output_version: VersionOut::Version(Self::VERSION.min(*input_version)),
                };
            }
            Interface::MsgWait { .. } => {
                // Receiving this message for the first time means that SPMC init succeeded
                self.switch_spmc_local_state(SpmcState::Boot, SpmcState::Runtime);

                // In this case the FFA_MSG_WAIT message shouldn't be forwarded, because this is not
                // a response to a call made by NWd.
                return (false, World::NonSecure);
            }
            Interface::SecondaryEpRegister { entrypoint } => {
                // TODO: check if the entrypoint is within the range of the SPMC's memory range
                // TODO: return Denied error if this is called on a secondary core
                let secondary_ep = match entrypoint {
                    SecondaryEpRegisterAddr::Addr32(addr) => *addr as usize,
                    SecondaryEpRegisterAddr::Addr64(addr) => *addr as usize,
                };
                self.spmc_secondary_ep.store(secondary_ep, Relaxed);
                *msg = Interface::success32_noargs()
            }
            Interface::Features { .. }
            | Interface::IdGet
            | Interface::SpmIdGet
            | Interface::PartitionInfoGetRegs { .. } => {
                return self.handle_secure_call_common(msg);
            }
            _ => {
                warn!("Denied FF-A call from Secure World: {msg:x?}");
                *msg = Interface::error(FfaError::Denied)
            }
        };

        (true, World::Secure)
    }

    /// Handles calls originating from the secure world during normal runtime operation.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_secure_call_runtime(&self, msg: &mut Interface) -> (bool, World) {
        // By default return to the same world
        let mut next_world = World::Secure;

        match msg {
            Interface::NormalWorldResume => {
                // Normal world execution was not preempted
                *msg = Interface::error(FfaError::Denied);
            }
            Interface::MsgSendDirectResp {
                src_id,
                dst_id,
                args,
            } => {
                if !Self::is_secure_id(*src_id)
                    || (Self::is_secure_id(*dst_id) && *dst_id != Self::OWN_ID)
                {
                    *msg = Interface::error(FfaError::InvalidParameters);
                } else if *dst_id == Self::OWN_ID {
                    *msg = match *args {
                        DirectMsgArgs::VersionResp { version } if *src_id == self.spmc_id => {
                            next_world = World::NonSecure;
                            Interface::VersionOut {
                                output_version: match version {
                                    None => VersionOut::NotSupported,
                                    Some(v) => VersionOut::Version(v),
                                },
                            }
                        }
                        _ => Interface::error(FfaError::InvalidParameters),
                    };
                } else {
                    // Forward to NWd
                    next_world = World::NonSecure;
                }
            }
            Interface::MsgSendDirectResp2 { src_id, dst_id, .. } => {
                if !Self::is_secure_id(*src_id) || Self::is_secure_id(*dst_id) {
                    *msg = Interface::error(FfaError::InvalidParameters)
                } else {
                    // Forward to NWd
                    next_world = World::NonSecure;
                }
            }
            Interface::Features { .. }
            | Interface::IdGet
            | Interface::SpmIdGet
            | Interface::PartitionInfoGetRegs { .. } => {
                return self.handle_secure_call_common(msg);
            }
            Interface::Error { .. }
            | Interface::Success { .. }
            | Interface::Interrupt { .. }
            | Interface::MsgWait { .. }
            | Interface::Yield
            | Interface::MemRetrieveResp { .. }
            | Interface::MemOpPause { .. }
            | Interface::MemFragRx { .. }
            | Interface::MemFragTx { .. } => {
                // Forward to NWd
                next_world = World::NonSecure;
            }
            _ => {
                warn!("Unsupported FF-A call from Secure World: {msg:x?}");
                *msg = Interface::error(FfaError::NotSupported)
            }
        };

        (true, next_world)
    }

    /// Handles calls originating from the secure world during secure interrupt handling.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_secure_call_interrupt(&self, msg: &mut Interface) -> (bool, World) {
        match msg {
            Interface::NormalWorldResume => {
                self.switch_spmc_local_state(SpmcState::SecureInterrupt, SpmcState::Runtime);

                // Interrupt was handled, return to NWd which was preempted by a secure interrupt.
                // Instead of forwarding the FFA_NORMAL_WORLD_RESUME message, NWd must be resumed
                // without any modification to its context. Returning None here will be converted to
                // SmcReturn::EMPTY by handle_secure_smc(), which means that no register will get
                // overwritten in NWd's context.
                (false, World::NonSecure)
            }
            _ => {
                warn!("Denied FF-A call from Secure World: {msg:x?}");
                *msg = Interface::error(FfaError::Denied);
                (true, World::Secure)
            }
        }
    }

    /// Handles calls originating from the secure world during PSCI event handling.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_secure_call_psci_event(&self, msg: &mut Interface) -> (bool, World) {
        match msg {
            Interface::MsgSendDirectResp {
                src_id,
                dst_id: Self::OWN_ID,
                args: DirectMsgArgs::PowerPsciResp { psci_status },
            } if *src_id == self.spmc_id => {
                if *psci_status != 0 {
                    warn!("PSCI response from SPMC: {psci_status}")
                }

                self.switch_spmc_local_state(SpmcState::PsciEventHandling, SpmcState::Runtime);

                (false, World::NonSecure)
            }
            _ => {
                warn!("Denied FF-A call from Secure World: {msg:x?}");
                *msg = Interface::error(FfaError::Denied);
                (true, World::Secure)
            }
        }
    }

    /// Handles calls originating from the non-secure world.
    /// The first return value indicates whether the `msg` is valid and needs to be serialized into
    /// the registers. The second return value specifies the next world to be called.
    fn handle_non_secure_call(&self, msg: &mut Interface) -> World {
        // By default return to the same world
        let mut next_world = World::NonSecure;

        match msg {
            Interface::Version { input_version } => {
                // Forward version call to the SPMC
                next_world = World::Secure;
                *msg = Interface::MsgSendDirectReq {
                    src_id: Self::OWN_ID,
                    dst_id: self.spmc_id,
                    args: DirectMsgArgs::VersionReq {
                        version: *input_version,
                    },
                };
            }
            Interface::IdGet => {
                // Return Hypervisor / NWd kernel endpoint ID
                *msg = Interface::Success {
                    target_info: TargetInfo::default(),
                    args: SuccessArgsIdGet { id: Self::NS_EP_ID }.into(),
                };
            }
            Interface::SpmIdGet => {
                *msg = Interface::Success {
                    target_info: TargetInfo::default(),
                    args: SuccessArgsSpmIdGet { id: self.spmc_id }.into(),
                };
            }
            Interface::MsgSendDirectReq { src_id, dst_id, .. }
            | Interface::MsgSendDirectReq2 { src_id, dst_id, .. } => {
                if Self::is_secure_id(*src_id) || !Self::is_secure_id(*dst_id) {
                    *msg = Interface::error(FfaError::InvalidParameters)
                } else {
                    next_world = World::Secure;
                }
            }
            Interface::MsgSend2 {
                sender_vm_id: src_id,
                ..
            } => {
                if Self::is_secure_id(*src_id) {
                    *msg = Interface::error(FfaError::InvalidParameters)
                } else {
                    next_world = World::Secure;
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
            | Interface::NotificationBitmapCreate { .. }
            | Interface::NotificationBitmapDestroy { .. }
            | Interface::NotificationBind { .. }
            | Interface::NotificationUnbind { .. }
            | Interface::NotificationSet { .. }
            | Interface::NotificationGet { .. }
            | Interface::NotificationInfoGet { .. }
            | Interface::MemDonate { .. }
            | Interface::MemLend { .. }
            | Interface::MemShare { .. }
            | Interface::MemRetrieveReq { .. }
            | Interface::MemReclaim { .. }
            | Interface::MemOpResume { .. }
            | Interface::MemFragRx { .. }
            | Interface::MemFragTx { .. } => {
                // Forward to SWd
                next_world = World::Secure;
            }
            _ => {
                warn!("Unsupported FF-A call from Normal World: {msg:x?}");
                *msg = Interface::error(FfaError::NotSupported)
            }
        };

        next_world
    }

    pub fn forward_secure_interrupt(&self, regs: &mut SmcReturn) -> World {
        let msg = Interface::Interrupt {
            // The endpoint and vCPU ID fields MBZ in this case
            target_info: TargetInfo {
                endpoint_id: 0,
                vcpu_id: 0,
            },
            // The SPMD shouldn't query the GIC
            interrupt_id: 0,
        };

        self.switch_spmc_local_state(SpmcState::Runtime, SpmcState::SecureInterrupt);

        let out_regs = regs.mark_all_used();
        msg.to_regs(self.spmc_version, out_regs);

        World::Secure
    }

    /// Notify the SPM that the current core was turned on for the first time or after CPU_OFF.
    pub fn handle_wake_from_cpu_off(&self) {
        self.switch_spmc_local_state(SpmcState::Off, SpmcState::Boot);
    }

    /// Notify the SPM that the current core woke up from suspend (CPU_SUSPEND, CPU_DEFAULT_SUSPEND
    /// or SYSTEM_SUSPEND). Only applies for power down suspend states.
    pub fn handle_wake_from_cpu_suspend(&self) -> SmcReturn {
        let msg = Interface::MsgSendDirectReq {
            src_id: Self::OWN_ID,
            dst_id: self.spmc_id,
            args: DirectMsgArgs::PowerWarmBootReq {
                // TODO: what is the use case for WarmBootType::ExitFromLowPower?
                boot_type: WarmBootType::ExitFromSuspend,
            },
        };

        self.switch_spmc_local_state(SpmcState::Runtime, SpmcState::PsciEventHandling);

        let mut out_regs = SmcReturn::EMPTY;
        msg.to_regs(self.spmc_version, out_regs.mark_all_used());

        out_regs
    }

    /// Return true if the FF-A endpoint ID is assigned to the secure world.
    pub const fn is_secure_id(id: u16) -> bool {
        id & 0x8000 != 0
    }
}

impl PsciSpmInterface for Spmd {
    fn forward_psci_request(&self, psci_request: &[u64; 4]) -> u64 {
        let version = self.spmc_version;
        let mut regs = SmcReturn::EMPTY;

        let msg = Interface::MsgSendDirectReq {
            src_id: Self::OWN_ID,
            dst_id: self.spmc_id,
            args: DirectMsgArgs::PowerPsciReq64 {
                params: *psci_request,
            },
        };

        msg.to_regs(version, regs.mark_all_used());

        switch_world(World::NonSecure, World::Secure);

        let ret: i32 = loop {
            match enter_world(&mut regs, World::Secure) {
                RunResult::Smc => match Interface::from_regs(version, regs.values()) {
                    Ok(Interface::MsgSendDirectResp {
                        src_id,
                        dst_id: Self::OWN_ID,
                        args: DirectMsgArgs::PowerPsciResp { psci_status },
                    }) if src_id == self.spmc_id => break psci_status,
                    _ => panic!("Unexpected SMC return from forwarding a PSCI request"),
                },
                // Interrupts shouldn't be routed to EL3 from SWd
                RunResult::Interrupt => panic!(
                    "Unexpected SMC return from forwarding a PSCI request - Interrupts shouldn't be routed to EL3 from SWd"
                ),
                RunResult::SysregTrap { .. } => todo!("Handle SysregTrap"),
            }
        };

        switch_world(World::Secure, World::NonSecure);

        ret as u64
    }

    fn notify_cpu_off(&self) {
        self.switch_spmc_local_state(SpmcState::Runtime, SpmcState::Off);
    }
}

#[cfg(test)]
pub struct TestSpm;

#[cfg(test)]
impl PsciSpmInterface for TestSpm {
    fn forward_psci_request(&self, _psci_request: &[u64; 4]) -> u64 {
        0
    }

    fn notify_cpu_off(&self) {}
}
