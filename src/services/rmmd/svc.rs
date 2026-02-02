// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

// This module contains definitions for the RMM-EL3 interface, which are not yet all used by RF-A.
#![allow(dead_code)]

use num_enum::TryFromPrimitive;

use crate::smccc::{SetFrom, SmcReturn};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnrecognisedFunctionId(u32),
    InvalidEccCurve(u64),
    InvalidEl3TokenSignOpcode(u64),
    InvalidMecRefreshReason(u64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[repr(i32)]
pub enum RmmCommandReturnCode {
    Ok = 0,
    Unk = -1,
    BadAddress = -2,
    BadPas = -3,
    NoMemory = -4,
    InvalidValue = -5,
    Again = -6,
}

impl From<RmmCommandReturnCode> for u64 {
    fn from(value: RmmCommandReturnCode) -> Self {
        // Casts as i64 to sign extend, then as u64 which is a no-op.
        // See https://doc.rust-lang.org/reference/expressions/operator-expr.html#semantics.
        (value as i64) as u64
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidEccCurve))]
#[repr(u64)]
pub enum EccCurve {
    EccSecp384r1 = 0,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidEl3TokenSignOpcode))]
#[repr(u64)]
pub enum El3TokenSignOpcode {
    Push = 0x1,
    Pull = 0x2,
    GetRak = 0x3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, TryFromPrimitive)]
#[num_enum(error_type(name = Error, constructor = Error::InvalidMecRefreshReason))]
#[repr(u64)]
pub enum MecRefreshReason {
    RealmCreation = 0,
    RealmDestruction = 1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamSelector {
    pub keyset: bool,
    pub dir: bool,
    pub substream: u8,
    pub stream_id: u8,
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
pub enum RmmFuncId {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RmmCall {
    RmiReqComplete {
        regs: [u64; 6],
    },
    GtsiDelegate {
        base_pa: usize,
    },
    GtsiUndelegate {
        base_pa: usize,
    },
    AttestGetRealmKey {
        buf_pa: usize,
        buf_size: usize,
        ecc_curve: EccCurve,
    },
    AttestGetPlatToken {
        buf_pa: usize,
        buf_size: usize,
        c_size: usize,
    },
    El3Features {
        feat_reg_idx: u64,
    },
    El3TokenSign {
        opcode: El3TokenSignOpcode,
        buf_pa: usize,
        buf_size: usize,
        ecc_curve: EccCurve,
    },
    MecRefresh {
        mecid: u16,
        reason: MecRefreshReason,
    },
    IdeKeyProg {
        ecam_address: u64,
        rp_id: u64,
        stream: StreamSelector,
        keq_qw: [u64; 4],
        ifv_qw: [u64; 2],
        request_id: u64,
        cookie: u64,
    },
    IdeKeySetGo {
        ecam_address: u64,
        rp_id: u64,
        stream: StreamSelector,
        request_id: u64,
        cookie: u64,
    },
    IdeKeySetStop {
        ecam_address: u64,
        rp_id: u64,
        stream: StreamSelector,
        request_id: u64,
        cookie: u64,
    },
    IdeKmPullResponse {
        ecam_address: u64,
        rp_id: u64,
    },
    ReserveMemory {
        size: usize,
        alignment: u8,
        local_cpu: bool,
    },
}

impl RmmCall {
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

pub struct RmmEmptyResponse;
derive_setfrom!(RmmEmptyResponse);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmAttestGetRealmKeyResponse {
    pub key_size: usize,
}
derive_setfrom!(RmmAttestGetRealmKeyResponse, key_size);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmAttestGetPlatTokenResponse {
    pub token_hunk_size: usize,
    pub remaining_size: usize,
}
derive_setfrom!(
    RmmAttestGetPlatTokenResponse,
    token_hunk_size,
    remaining_size
);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmEl3FeaturesResponse {
    pub feat_reg: u64,
}
derive_setfrom!(RmmEl3FeaturesResponse, feat_reg);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmEl3TokenSignGetRakResponse {
    pub key_size: u64,
}
derive_setfrom!(RmmEl3TokenSignGetRakResponse, key_size);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmIdeKmPullResponse {
    pub previous: RmmCommandReturnCode,
    pub r1: u64,
    pub r2: u64,
}
derive_setfrom!(RmmIdeKmPullResponse, previous, r1, r2);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RmmReserveMemoryResponse {
    pub address: usize,
}
derive_setfrom!(RmmReserveMemoryResponse, address);
