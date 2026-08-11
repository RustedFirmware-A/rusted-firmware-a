// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Definitions for the Realm Management Interface based on document DEN0137 version 2.0-bet3.

#![cfg_attr(not(test), no_std)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(missing_docs)]

use num_enum::TryFromPrimitive;

/// Internal error type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Function ID is not defined by the RMI interface.
    UnknownFunctionId(u32),
    /// Function ID is defined by RMI but not parsed by this crate.
    Unsupported,
}

/// RMI return codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum RmiStatusCode {
    /// Command completed successfully, leaving no intermediate state
    Success = 0,
    /// The value of a command input value caused the command to fail
    ErrorInput = 1,
    /// An attribute of a Realm does not match the expected value
    ErrorRealm = 2,
    /// An attribute of a Rec does not match the expected value
    ErrorRec = 3,
    /// An RTT walk terminated before reaching the target RTT level, or reached an RTTE with an
    /// unexpected value
    ErrorRtt = 4,
    /// The command is not supported
    ErrorNotSupported = 5,
    /// An attribute of a device does not match the expected value
    ErrorDevice = 6,
    /// RTTE in an auxiliary RTT contained an unexpected value
    ErrorRttAux = 7,
    /// A PSMMU Stream Table walk terminated before reaching the target level, or reached an entry
    /// with an unexpected value
    ErrorPSmmuSt = 8,
    /// A DPT walk terminated before reaching the target level, or reached an entry with an
    /// unexpected value
    ErrorDpt = 9,
    /// The command failed to make progress, for an IMPLEMENTATION DEFINED reason.
    /// The reason for the lack of progress may be temporary, and may be resolved without requiring
    /// any action to be taken by the Host.
    /// The command did not result in any changes of system state.
    Busy = 10,
    /// An attribute of RMM global state does not match the expected value
    ErrorGlobal = 11,
    /// The state of a tracking region does not match the expected value
    ErrorTracking = 12,
    /// The command initiated a state transition but did not complete, leaving an object in an
    /// intermediate state. The target object cannot be the subject of any other RMI command, until
    /// this one has been completed.
    Incomplete = 13,
    /// The command failed to make progress due to a target object being in an intermediate state,
    /// or due to the platform having insufficient resources to initiate a Stateful RMI Operation.
    /// An incomplete operation must be completed before this one can proceed.
    /// The command did not result in any changes of system state.
    Blocked = 14,
    /// A GPT walk terminated before reaching the target level, or reached an entry with an unexpected
    /// value
    ErrorGpt = 15,
    /// An attribute of a Granule does not match the expected value
    ErrorGranule = 16,
}

/// RMI function ids
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::UnknownFunctionId))]
#[repr(u32)]
pub enum RmiFuncId {
    /// RMI_VERSION command.
    Version = 0xC400_0150,
    /// RMI_RTT_DATA_MAP_INIT command.
    RttDataMapInit = 0xC400_0153,
    /// RMI_REALM_ACTIVATE command.
    RealmActivate = 0xC400_0157,
    /// RMI_REALM_CREATE command.
    RealmCreate = 0xC400_0158,
    /// RMI_REALM_DESTROY command.
    RealmDestroy = 0xC400_0159,
    /// RMI_REC_CREATE command.
    RecCreate = 0xC400_015A,
    /// RMI_REC_DESTROY command.
    RecDestroy = 0xC400_015B,
    /// RMI_REC_ENTER command.
    RecEnter = 0xC400_015C,
    /// RMI_RTT_CREATE command.
    RttCreate = 0xC400_015D,
    /// RMI_RTT_DESTROY command.
    RttDestroy = 0xC400_015E,
    /// RMI_RTT_READ_ENTRY command.
    RttReadEntry = 0xC400_0161,
    /// RMI_RTT_DEV_VALIDATE command.
    RttDevValidate = 0xC400_0163,
    /// RMI_PSCI_COMPLETE command.
    PsciComplete = 0xC400_0164,
    /// RMI_FEATURES command.
    Features = 0xC400_0165,
    /// RMI_RTT_FOLD command.
    RttFold = 0xC400_0166,
    /// RMI_RTT_INIT_RIPAS command.
    RttInitRipas = 0xC400_0168,
    /// RMI_RTT_SET_RIPAS command.
    RttSetRipas = 0xC400_0169,
    /// RMI_VSMMU_CREATE command.
    VsmmuCreate = 0xC400_016A,
    /// RMI_VSMMU_DESTROY command.
    VsmmuDestroy = 0xC400_016B,
    /// RMI_RMM_CONFIG_SET command.
    RmmConfigSet = 0xC400_016E,
    /// RMI_PSMMU_IRQ_NOTIFY command.
    PsmmuIrqNotify = 0xC400_016F,
    /// RMI_ATTEST_PLAT_TOKEN_REFRESH command.
    AttestPlatTokenRefresh = 0xC400_0170,
    /// RMI_PDEV_ABORT command.
    PdevAbort = 0xC400_0174,
    /// RMI_PDEV_COMMUNICATE command.
    PdevCommunicate = 0xC400_0175,
    /// RMI_PDEV_CREATE command.
    PdevCreate = 0xC400_0176,
    /// RMI_PDEV_DESTROY command.
    PdevDestroy = 0xC400_0177,
    /// RMI_PDEV_GET_STATE command.
    PdevGetState = 0xC400_0178,
    /// RMI_PDEV_STREAM_KEY_REFRESH command.
    PdevStreamKeyRefresh = 0xC400_017A,
    /// RMI_PDEV_SET_PUBKEY command.
    PdevSetPubkey = 0xC400_017B,
    /// RMI_PDEV_STOP command.
    PdevStop = 0xC400_017C,
    /// RMI_RTT_AUX_CREATE command.
    RttAuxCreate = 0xC400_017D,
    /// RMI_RTT_AUX_DESTROY command.
    RttAuxDestroy = 0xC400_017E,
    /// RMI_RTT_AUX_FOLD command.
    RttAuxFold = 0xC400_017F,
    /// RMI_VDEV_ABORT command.
    VdevAbort = 0xC400_0185,
    /// RMI_VDEV_COMMUNICATE command.
    VdevCommunicate = 0xC400_0186,
    /// RMI_VDEV_CREATE command.
    VdevCreate = 0xC400_0187,
    /// RMI_VDEV_DESTROY command.
    VdevDestroy = 0xC400_0188,
    /// RMI_VDEV_GET_STATE command.
    VdevGetState = 0xC400_0189,
    /// RMI_VDEV_UNLOCK command.
    VdevUnlock = 0xC400_018A,
    /// RMI_RTT_SET_S2AP command.
    RttSetS2ap = 0xC400_018B,
    /// RMI_VDEV_GET_INTERFACE_REPORT command.
    VdevGetInterfaceReport = 0xC400_01D0,
    /// RMI_VDEV_GET_MEASUREMENTS command.
    VdevGetMeasurements = 0xC400_01D1,
    /// RMI_VDEV_LOCK command.
    VdevLock = 0xC400_01D2,
    /// RMI_VDEV_START command.
    VdevStart = 0xC400_01D3,
    /// RMI_VSMMU_EVENT_HANDLE command.
    VsmmuEventHandle = 0xC400_01D6,
    /// RMI_PSMMU_ACTIVATE command.
    PsmmuActivate = 0xC400_01D7,
    /// RMI_PSMMU_DEACTIVATE command.
    PsmmuDeactivate = 0xC400_01D8,
    /// RMI_PSMMU_ST_L2_CREATE command.
    PsmmuStL2Create = 0xC400_01DB,
    /// RMI_PSMMU_ST_L2_DESTROY command.
    PsmmuStL2Destroy = 0xC400_01DC,
    /// RMI_DPT_L0_CREATE command.
    DptL0Create = 0xC400_01DD,
    /// RMI_DPT_L0_DESTROY command.
    DptL0Destroy = 0xC400_01DE,
    /// RMI_DPT_L1_CREATE command.
    DptL1Create = 0xC400_01DF,
    /// RMI_DPT_L1_DESTROY command.
    DptL1Destroy = 0xC400_01E0,
    /// RMI_GRANULE_TRACKING_GET command.
    GranuleTrackingGet = 0xC400_01E1,
    /// RMI_GRANULE_TRACKING_SET command.
    GranuleTrackingSet = 0xC400_01E3,
    /// RMI_RMM_CONFIG_GET command.
    RmmConfigGet = 0xC400_01EC,
    /// RMI_RMM_STATE_GET command.
    RmmStateGet = 0xC400_01EE,
    /// RMI_PSMMU_EVENT_CONSUME command.
    PsmmuEventConsume = 0xC400_01F0,
    /// RMI_GRANULE_RANGE_DELEGATE command.
    GranuleRangeDelegate = 0xC400_01F1,
    /// RMI_GRANULE_RANGE_UNDELEGATE command.
    GranuleRangeUndelegate = 0xC400_01F2,
    /// RMI_GPT_L1_CREATE command.
    GptL1Create = 0xC400_01F3,
    /// RMI_GPT_L1_DESTROY command.
    GptL1Destroy = 0xC400_01F4,
    /// RMI_RTT_DATA_MAP command.
    RttDataMap = 0xC400_01F5,
    /// RMI_RTT_DATA_UNMAP command.
    RttDataUnmap = 0xC400_01F6,
    /// RMI_RTT_DEV_MAP command.
    RttDevMap = 0xC400_01F7,
    /// RMI_RTT_DEV_UNMAP command.
    RttDevUnmap = 0xC400_01F8,
    /// RMI_RTT_ARCH_DEV_MAP command.
    RttArchDevMap = 0xC400_01F9,
    /// RMI_RTT_ARCH_DEV_UNMAP command.
    RttArchDevUnmap = 0xC400_01FA,
    /// RMI_RTT_UNPROT_MAP command.
    RttUnprotMap = 0xC400_01FB,
    /// RMI_RTT_UNPROT_UNMAP command.
    RttUnprotUnmap = 0xC400_01FC,
    /// RMI_RTT_AUX_PROT_MAP command.
    RttAuxProtMap = 0xC400_01FD,
    /// RMI_RTT_AUX_PROT_UNMAP command.
    RttAuxProtUnmap = 0xC400_01FE,
    /// RMI_RTT_AUX_UNPROT_MAP command.
    RttAuxUnprotMap = 0xC400_01FF,
    /// RMI_RTT_AUX_UNPROT_UNMAP command.
    RttAuxUnprotUnmap = 0xC400_0200,
    /// RMI_REALM_TERMINATE command.
    RealmTerminate = 0xC400_0201,
    /// RMI_RMM_ACTIVATE command.
    RmmActivate = 0xC400_0202,
    /// RMI_OP_CONTINUE command.
    OpContinue = 0xC400_0203,
    /// RMI_PDEV_STREAM_CONNECT command.
    PdevStreamConnect = 0xC400_0204,
    /// RMI_PDEV_STREAM_DISCONNECT command.
    PdevStreamDisconnect = 0xC400_0205,
    /// RMI_PDEV_STREAM_COMPLETE command.
    PdevStreamComplete = 0xC400_0206,
    /// RMI_PDEV_STREAM_KEY_PURGE command.
    PdevStreamKeyPurge = 0xC400_0207,
    /// RMI_OP_MEM_DONATE command.
    OpMemDonate = 0xC400_0208,
    /// RMI_OP_MEM_RECLAIM command.
    OpMemReclaim = 0xC400_0209,
    /// RMI_OP_CANCEL command.
    OpCancel = 0xC400_020A,
    /// RMI_VSMMU_FEATURES command.
    VsmmuFeatures = 0xC400_020B,
    /// RMI_VSMMU_CMD_GET command.
    VsmmuCmdGet = 0xC400_020C,
    /// RMI_VSMMU_CMD_COMPLETE command.
    VsmmuCmdComplete = 0xC400_020D,
    /// RMI_PSMMU_INFO command.
    PsmmuInfo = 0xC400_020E,
    /// RMI_RMM_DEACTIVATE command.
    RmmDeactivate = 0xC400_020F,
    /// RMI_PDEV_STREAM_INFO command.
    PdevStreamInfo = 0xC400_0210,
    /// RMI_GPT_INFO command.
    GptInfo = 0xC400_0211,
}

/// RMI functions and parameters
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RmiFunction {
    /// Parsed RMI_VERSION command.
    Version {
        /// Requested interface revision
        req: u64,
    },
    /// Parsed RMI_GRANULE_RANGE_DELEGATE command.
    GranuleRangeDelegate {
        /// Base PA of the target range
        base_pa: u64,
        /// Top PA of the target range
        top_pa: u64,
    },
    /// Parsed RMI_GRANULE_RANGE_UNDELEGATE command.
    GranuleRangeUndelegate {
        /// Base PA of the target range
        base_pa: u64,
        /// Top PA of the target range
        top_pa: u64,
    },
}

impl TryFrom<&[u64; 18]> for RmiFunction {
    type Error = Error;

    fn try_from(regs: &[u64; 18]) -> Result<Self, Self::Error> {
        let fid = RmiFuncId::try_from(regs[0] as u32)?;

        match fid {
            RmiFuncId::Version => Ok(Self::Version { req: regs[1] }),
            RmiFuncId::GranuleRangeDelegate => Ok(Self::GranuleRangeDelegate {
                base_pa: regs[1],
                top_pa: regs[2],
            }),
            RmiFuncId::GranuleRangeUndelegate => Ok(Self::GranuleRangeUndelegate {
                base_pa: regs[1],
                top_pa: regs[2],
            }),
            _ => Err(Error::Unsupported),
        }
    }
}
