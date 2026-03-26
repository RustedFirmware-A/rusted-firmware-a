// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Definitions for the RMM-EL3 interface.

// This module contains definitions for the RMM-EL3 interface, which are not yet all used by RF-A.
#![allow(dead_code)]

use num_enum::TryFromPrimitive;

use crate::smccc::{SetFrom, SmcReturn};

/// A function ID or parameter value was invalid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// The function ID was not recognised.
    UnrecognisedFunctionId(u32),
    /// The ECC curve was not recognised.
    InvalidEccCurve(u64),
    /// The RMM_EL3_TOKEN_SIGN opcode was not recognised.
    InvalidEl3TokenSignOpcode(u64),
    /// The RMM_MEC_REFRESH reason was not recognised.
    InvalidMecRefreshReason(u64),
}

/// The status returned by an RMM command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(i32)]
pub enum RmmCommandReturnCode {
    /// No errors detected.
    Ok = 0,
    /// Unknown/Generic error.
    Unk = -1,
    /// The value of an address used as argument was invalid.
    BadAddress = -2,
    /// Incorrect PAS.
    BadPas = -3,
    /// Not enough memory to perform an operation.
    NoMemory = -4,
    /// The value of an argument was invalid.
    InvalidValue = -5,
    /// The resource is busy. Try again.
    Again = -6,
}

impl From<RmmCommandReturnCode> for u64 {
    fn from(value: RmmCommandReturnCode) -> Self {
        // Casts as i64 to sign extend, then as u64 which is a no-op.
        // See https://doc.rust-lang.org/reference/expressions/operator-expr.html#semantics.
        (value as i64) as u64
    }
}

/// Which ECC curve to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidEccCurve))]
#[repr(u64)]
pub enum EccCurve {
    /// The ECC P384 curve.
    EccSecp384r1 = 0,
}

/// Opcode for the RMM_EL3_TOKEN_SIGN command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidEl3TokenSignOpcode))]
#[repr(u64)]
pub enum El3TokenSignOpcode {
    /// RMM_EL3_TOKEN_SIGN_PUSH_REQ_OP
    Push = 0x1,
    /// RMM_EL3_TOKEN_SIGN_PULL_RESP_OP
    Pull = 0x2,
    /// RMM_EL3_TOKEN_SIGN_GET_RAK_PUB_OP
    GetRak = 0x3,
}

/// Reason for an RMM_MEC_REFRESH command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidMecRefreshReason))]
#[repr(u64)]
pub enum MecRefreshReason {
    /// Realm creation.
    RealmCreation = 0,
    /// Realm destruction.
    RealmDestruction = 1,
}

/// IDE selective stream information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSelector {
    keyset: bool,
    dir: bool,
    substream: u8,
    stream_id: u8,
}

impl From<u64> for StreamSelector {
    fn from(value: u64) -> Self {
        Self {
            keyset: value & (1 << 12) != 0,
            dir: value & (1 << 11) != 0,
            substream: ((value >> 8) & 0b111) as u8,
            stream_id: value as u8,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::UnrecognisedFunctionId))]
#[repr(u32)]
enum RmmFuncId {
    RmiReqComplete = 0xC400_018F,
    GtsiDelegate = 0xC400_01B0,
    GtsiUndelegate = 0xC400_01B1,
    AttestGetRealmKey = 0xC400_01B2,
    AttestGetPlatToken = 0xC400_01B3,
    El3Features = 0xC400_01B4,
    El3TokenSign = 0xC400_01B5,
    MecRefresh = 0xC400_01B6,
    IdeKeyProg = 0xC400_01B7,
    IdeKeySetGo = 0xC400_01B8,
    IdeKeySetStop = 0xC400_01B9,
    IdeKmPullResponse = 0xC400_01BA,
    ReserveMemory = 0xC400_01BB,
}

/// A call to an RMM function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RmmCall {
    /// Notifies the completion of an RMI call to the Non-Secure world.
    RmiReqComplete {
        /// Register values.
        regs: [u64; 6],
    },
    /// Delegates a memory granule by changing its PAS from Non-Secure to Realm.
    GtsiDelegate {
        /// Physical address of the start of the granule to be delegated.
        base_pa: usize,
    },
    /// Undelegates a memory granule by changing its PAS from Realm to Non-Secure.
    GtsiUndelegate {
        /// Physical address of the start of the granule to be undelegated.
        base_pa: usize,
    },
    /// Retrieves the Realm Attestation Token Signing key from EL3.
    AttestGetRealmKey {
        /// Physical address where the Realm Attestation Key must be stored by EL3. The PA must
        /// belong to the shared buffer.
        buf_pa: usize,
        /// Size in bytes of the Realm Attestation Key buffer. `buf_pa` + `buf_size` must lie within
        /// the shared buffer.
        buf_size: usize,
        /// Type of the elliptic curve to which the requested attestation key belongs.
        ecc_curve: EccCurve,
    },
    /// Retrieves the Platform Token from EL3.
    AttestGetPlatToken {
        /// Physical address of the platform attestation token.
        buf_pa: usize,
        /// Size in bytes of the platform attestation token buffer. `buf_pa` + `buf_size` must lie
        /// within the shared buffer.
        buf_size: usize,
        /// Size in bytes of the challenge object. It corresponds to the size of one of the defined
        /// SHA algorithms. Any subsequent calls, if required to retrieve the full token, should set
        /// this size to 0.
        c_size: usize,
    },
    /// Provides a mechanism to discover features and ABIs supported by the RMM-EL3 interface, for a
    /// given version.
    El3Features {
        /// Feature register index.
        feat_reg_idx: u64,
    },
    /// Sends requests related to realm attestation token signing requests to EL3.
    El3TokenSign {
        /// The operation to send.
        opcode: El3TokenSignOpcode,
        /// Physical address for the request or response, depending on the opcode.
        buf_pa: usize,
        /// Size in bytes of the buffer in `buf_pa`. `buf_pa` + `buf_size` must lie within the
        /// shared buffer.
        buf_size: usize,
        /// Type of the elliptic curve to which the requested attestation key belongs.
        ecc_curve: EccCurve,
    },
    /// Updates the tweak for the encryption key/programs a new encryption key associated with a
    /// given MECID.
    MecRefresh {
        /// Identifies the MECID for which the encryption key is to be updated.
        mecid: u16,
        /// The reason for the MEC refresh.
        reason: MecRefreshReason,
    },
    /// Sets the key/IV info at Root port for an IDE stream as part of Device Assignment flow.
    IdeKeyProg {
        /// Used to identify the root complex.
        ecam_address: u64,
        /// Used to identify the root port within the root complex.
        rp_id: u64,
        /// IDE selective stream information.
        stream: StreamSelector,
        /// Quad words of key.
        keq_qw: [u64; 4],
        /// Quad words of IV.
        ifv_qw: [u64; 2],
        /// Used only in non-blocking mode. Ignored in blocking mode.
        request_id: u64,
        /// Used only in non-blocking mode. Ignored in blocking mode.
        cookie: u64,
    },
    /// Activates the IDE stream at Root Port once the keys have been programmed as part of Device
    /// Assignment flow.
    IdeKeySetGo {
        /// Used to identify the root complex.
        ecam_address: u64,
        /// Used to identify the root port within the root complex.
        rp_id: u64,
        /// IDE selective stream information.
        stream: StreamSelector,
        /// Used only in non-blocking mode. Ignored in blocking mode.
        request_id: u64,
        /// Used only in non-blocking mode. Ignored in blocking mode.
        cookie: u64,
    },
    /// Deactivates the IDE stream at Root Port as part of Device Assignment flow.
    IdeKeySetStop {
        /// Used to identify the root complex.
        ecam_address: u64,
        /// Used to identify the root port within the root complex.
        rp_id: u64,
        /// IDE selective stream information.
        stream: StreamSelector,
        /// Used only in non-blocking mode. Ignored in blocking mode.
        request_id: u64,
        /// Used only in non-blocking mode. Ignored in blocking mode.
        cookie: u64,
    },
    /// Retrieves the response from Root Port to a previous non-blocking IDE-KM SMC request as part
    /// of Device Assignment flow.
    IdeKmPullResponse {
        /// Used to identify the root complex.
        ecam_address: u64,
        /// Used to identify the root port within the root complex.
        rp_id: u64,
    },
    /// Reserves memory for the RMM, during RMM boot time.
    ReserveMemory {
        /// Required size of the memory region, in bytes.
        size: usize,
        /// Alignment requirement, in bits. A value of 16 would return a 64 KB aligned base address.
        alignment: u8,
        /// Determines whether the reservation should be taken from a pool close to the calling CPU.
        local_cpu: bool,
    },
}

impl RmmCall {
    /// Parses the given register values as an RMM call, or returns an error if the function ID is
    /// not recognised.
    pub fn from_regs(regs: &[u64]) -> Result<Self, Error> {
        let fid = RmmFuncId::try_from(regs[0] as u32)?;

        Ok(match fid {
            RmmFuncId::RmiReqComplete => Self::RmiReqComplete {
                regs: regs[1..7].try_into().unwrap(),
            },
            RmmFuncId::GtsiDelegate => Self::GtsiDelegate {
                base_pa: regs[1] as usize,
            },
            RmmFuncId::GtsiUndelegate => Self::GtsiUndelegate {
                base_pa: regs[1] as usize,
            },
            RmmFuncId::AttestGetRealmKey => Self::AttestGetRealmKey {
                buf_pa: regs[1] as usize,
                buf_size: regs[2] as usize,
                ecc_curve: regs[3].try_into()?,
            },
            RmmFuncId::AttestGetPlatToken => Self::AttestGetPlatToken {
                buf_pa: regs[1] as usize,
                buf_size: regs[2] as usize,
                c_size: regs[3] as usize,
            },
            RmmFuncId::El3Features => Self::El3Features {
                feat_reg_idx: regs[1],
            },
            RmmFuncId::El3TokenSign => Self::El3TokenSign {
                opcode: regs[1].try_into()?,
                buf_pa: regs[2] as usize,
                buf_size: regs[3] as usize,
                ecc_curve: regs[4].try_into()?,
            },
            RmmFuncId::MecRefresh => Self::MecRefresh {
                mecid: (regs[1] >> 32) as u16,
                reason: (regs[1] & 0x1).try_into()?,
            },
            RmmFuncId::IdeKeyProg => Self::IdeKeyProg {
                ecam_address: regs[1],
                rp_id: regs[2],
                stream: regs[3].into(),
                keq_qw: [regs[4], regs[5], regs[6], regs[7]],
                ifv_qw: [regs[8], regs[9]],
                request_id: regs[10],
                cookie: regs[11],
            },
            RmmFuncId::IdeKeySetGo => Self::IdeKeySetGo {
                ecam_address: regs[1],
                rp_id: regs[2],
                stream: regs[3].into(),
                request_id: regs[4],
                cookie: regs[5],
            },
            RmmFuncId::IdeKeySetStop => Self::IdeKeySetStop {
                ecam_address: regs[1],
                rp_id: regs[2],
                stream: regs[3].into(),
                request_id: regs[4],
                cookie: regs[5],
            },
            RmmFuncId::IdeKmPullResponse => Self::IdeKmPullResponse {
                ecam_address: regs[1],
                rp_id: regs[2],
            },
            RmmFuncId::ReserveMemory => Self::ReserveMemory {
                size: regs[1] as usize,
                alignment: (regs[2] >> 56) as u8,
                local_cpu: regs[2] & 0b1 == 0b1,
            },
        })
    }
}

macro_rules! count_values {
    () => {0};
    ($x:ident $(, $xs:ident)*) => {
        1 + count_values!($($xs),*)
    };
}
macro_rules! derive_setfrom {
    ($name:ident $(,$field:ident)*) => {
        impl SetFrom<$name> for SmcReturn {
            #[allow(unused_variables)]
            fn set_from(&mut self, value: $name) {
                let regs = self.mark_used::<{ 1 + count_values!($($field),*) }>();
                regs[0] = RmmCommandReturnCode::Ok.into();

                #[allow(unused)]
                let rem_regs = &mut regs[1..];
                $(
                    rem_regs[0] = value.$field as u64;
                    #[allow(unused)]
                    let rem_regs = &mut rem_regs[1..];
                )*

                rem_regs.fill(0);
            }
        }
    };
}

impl SetFrom<RmmCommandReturnCode> for SmcReturn {
    fn set_from(&mut self, value: RmmCommandReturnCode) {
        self.mark_used::<1>()[0] = value.into()
    }
}

struct RmmEmptyResponse;
derive_setfrom!(RmmEmptyResponse);

/// The response to an RMM ATTEST_GET_REALM_KEY request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmAttestGetRealmKeyResponse {
    /// Size in bytes of the Realm Attestation Key.
    pub key_size: usize,
}
derive_setfrom!(RmmAttestGetRealmKeyResponse, key_size);

/// The response to an RMM ATTEST_GET_PLAT_TOKEN request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmAttestGetPlatTokenResponse {
    /// Size of the platform token hunk retrieved.
    pub token_hunk_size: usize,
    /// Remaining bytes of the token that are pending retrieval.
    pub remaining_size: usize,
}
derive_setfrom!(
    RmmAttestGetPlatTokenResponse,
    token_hunk_size,
    remaining_size
);

/// The response to an RMM EL3_FEATURES request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmEl3FeaturesResponse {
    /// Value of the requested feature register.
    pub feat_reg: u64,
}
derive_setfrom!(RmmEl3FeaturesResponse, feat_reg);

/// The response to an RMM EL3_TOKEN_SIGN_GET_RAK request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RmmEl3TokenSignGetRakResponse {
    /// The length of public key returned.
    pub key_size: u64,
}
derive_setfrom!(RmmEl3TokenSignGetRakResponse, key_size);

/// The response to an RMM IDE_KM_PULL request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RmmIdeKmPullResponse {
    /// Retrieved response corresponding to previous IDE_KM requests.
    pub previous: RmmCommandReturnCode,
    /// Passthrough from requested SMC.
    pub r1: u64,
    /// Passthrough from requested SMC.
    pub r2: u64,
}
derive_setfrom!(RmmIdeKmPullResponse, previous, r1, r2);

/// The response to an RMM RESERVE_MEMORY request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RmmReserveMemoryResponse {
    /// Physical address of the reserved memory area.
    pub address: usize,
}
derive_setfrom!(RmmReserveMemoryResponse, address);
