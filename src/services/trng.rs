// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    context::World,
    platform::TrngPlatformImpl,
    services::{Service, owns},
    smccc::{FunctionId, OwningEntityNumber, SUCCESS, SmcReturn},
};
use spin::mutex::SpinMutex;
use uuid::Uuid;

// TRNG SMC function identifiers, as defined in the TRNG Firmware Interface
// specification, Arm DEN 0098.
const ARM_TRNG_VERSION: u32 = 0x8400_0050;
const ARM_TRNG_FEATURES: u32 = 0x8400_0051;
const ARM_TRNG_GET_UUID: u32 = 0x8400_0052;
const ARM_TRNG_RND32: u32 = 0x8400_0053;
const ARM_TRNG_RND64: u32 = 0xC400_0053;

// TRNG function number range
const TRNG_FN_NUM_MIN: u16 = 0x50;
const TRNG_FN_NUM_MAX: u16 = 0x53;

// TRNG spec version number
const TRNG_VERSION_MAJOR: u32 = 1;
const TRNG_VERSION_MINOR: u32 = 0;
const TRNG_VERSION: u32 = (TRNG_VERSION_MAJOR << 16) | TRNG_VERSION_MINOR;

/// TRNG error numbers
#[repr(i32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrngError {
    NotSupported = -1,
    InvalidParams = -2,
    #[allow(unused)]
    NoEntropy = -3,
}

impl From<TrngError> for SmcReturn {
    fn from(e: TrngError) -> Self {
        SmcReturn::from(e as i32)
    }
}

const TRNG_RND32_ENTROPY_MAXBITS: usize = 96;
const TRNG_RND64_ENTROPY_MAXBITS: usize = 192;

/// The TRNG Firmware Interface can request up to `TRNG_RND64_ENTROPY_MAXBITS`
/// bits of entropy per call, so the pool must hold at least that much, plus
/// enough space for one extra TRNG request in the worst case where the pool is
/// less than one word short of entropy.
const WORDS_IN_POOL: usize =
    TRNG_RND64_ENTROPY_MAXBITS / BITS_PER_WORD + TrngPlatformImpl::REQ_WORDS;
const BITS_PER_WORD: usize = 64;
const BITS_IN_POOL: usize = WORDS_IN_POOL * BITS_PER_WORD;

/// Platform-specific TRNG interface.
/// The platform must provide an implementation for this trait. If the platform
/// does not have a TRNG source, then it can use the default implementation,
/// `NotSupportedTrngPlatformImpl`
pub trait TrngPlatformInterface {
    /// A UUID for the entropy source, or nil (all-zero) if not implemented.
    const TRNG_UUID: Uuid = Uuid::nil();
    /// The number of 64-bit words per request from the TRNG.
    const REQ_WORDS: usize = 1;

    /// Perform any necessary platform-specific setup for the entropy source.
    fn entropy_setup() {}

    /// Get REQ_WORDS of 64-bit values from the platform's entropy source.
    fn get_entropy() -> Result<[u64; TrngPlatformImpl::REQ_WORDS], TrngError> {
        Err(TrngError::NotSupported)
    }
}

/// Default implementation of TrngPlatformInterface for platforms that do not
/// have a TRNG source.
pub struct NotSupportedTrngPlatformImpl;
impl TrngPlatformInterface for NotSupportedTrngPlatformImpl {}

/// Entropy pool is implemented with a ring buffer of bits, so that requests for
/// abitrary numbers of bits of entropy can be handled without throwing away the
/// leftover 1-63 bits of entropy.
struct EntropyPool {
    entropy: [u64; WORDS_IN_POOL],
    entropy_bit_index: usize,
    entropy_bit_size: usize,
}

impl EntropyPool {
    const fn new() -> Self {
        Self {
            entropy: [0; WORDS_IN_POOL],
            entropy_bit_index: 0,
            entropy_bit_size: 0,
        }
    }

    /// Fill the entropy pool until we have at least as many bits as requested.
    /// Returns Ok after filling the pool, and an error if the entropy source is
    /// out of entropy and the pool could not be filled.
    fn fill_entropy(&mut self, nbits: usize) -> Result<(), TrngError> {
        while nbits > self.entropy_bit_size {
            let buf = TrngPlatformImpl::get_entropy()?;
            let free_bit = self.entropy_bit_size + self.entropy_bit_index;
            let mut free_word = (free_bit / BITS_PER_WORD) % WORDS_IN_POOL;
            for val in buf {
                self.entropy[free_word] = val;
                free_word = (free_word + 1) % WORDS_IN_POOL;
                self.entropy_bit_size += BITS_PER_WORD;
            }
            assert!(self.entropy_bit_size <= BITS_IN_POOL);
        }
        Ok(())
    }

    /// Pack entropy into the out buffer, filling the entropy pool as needed.
    /// Returns Ok on success, and an error on failure.
    /// Note: out must have enough space for nbits of entropy
    fn pack_entropy(&mut self, nbits: usize, out: &mut [u64]) -> Result<(), TrngError> {
        self.fill_entropy(nbits)?;

        for x in out.iter_mut() {
            *x = 0;
        }

        let rshift = self.entropy_bit_index % BITS_PER_WORD;
        let lshift = BITS_PER_WORD - rshift;
        let to_fill = nbits.div_ceil(BITS_PER_WORD);
        let mut bits_to_discard = nbits;

        for (idx, word_i) in out.iter_mut().enumerate().take(to_fill) {
            // Repack the entropy from the pool into the passed in out
            // buffer. This takes lesser bits from the valid upper bits
            // of word_i and more bits from the lower bits of (word_i + 1).
            //
            // In the following diagram, `e` represents
            // valid entropy, ` ` represents invalid bits (not entropy) and
            // `x` represents valid entropy that must not end up in the
            // packed word.
            //
            //          |---------entropy pool----------|
            // variable |--(word_i + 1)-|----word_i-----|
            // bit idx  |7 6 5 4 3 2 1 0|7 6 5 4 3 2 1 0|
            //          [x,x,e,e,e,e,e,e|e,e, , , , , , ]
            //          |   [e,e,e,e,e,e,e,e]           |
            //          |   |--out[word_i]--|           |
            //    lshift|---|               |--rshift---|
            //
            //          ==== Which is implemented as ====
            //
            //          |---------entropy pool----------|
            // variable |--(word_i + 1)-|----word_i-----|
            // bit idx  |7 6 5 4 3 2 1 0|7 6 5 4 3 2 1 0|
            //          [x,x,e,e,e,e,e,e|e,e, , , , , , ]
            // expr         << lshift       >> rshift
            // bit idx   5 4 3 2 1 0                 7 6
            //          [e,e,e,e,e,e,0,0|0,0,0,0,0,0,e,e]
            //                ==== bit-wise or ====
            //                   5 4 3 2 1 0 7 6
            //                  [e,e,e,e,e,e,e,e]
            let min_word = self.entropy_bit_index / BITS_PER_WORD;
            let pool_idx0 = (min_word + idx) % WORDS_IN_POOL;
            *word_i |= self.entropy[pool_idx0] >> rshift;

            // Discarding the used/packed entropy bits from the respective
            // words, (word_i) and (word_i+1) as applicable.
            // In each iteration of the loop, we pack 64bits of entropy to
            // the output buffer. The bits are picked linearly starting from
            // 1st word (entropy[0]) till 4th word (entropy[3]) and then
            // rolls back (entropy[0]). Discarding of bits is managed
            // similarly.
            //
            // The following diagram illustrates the logic:
            //
            //          |---------entropy pool----------|
            // variable |--(word_i + 1)-|----word_i-----|
            // bit idx  |7 6 5 4 3 2 1 0|7 6 5 4 3 2 1 0|
            //          [e,e,e,e,e,e,e,e|e,e,0,0,0,0,0,0]
            //          |   [e,e,e,e,e,e,e,e]           |
            //          |   |--out[word_i]--|           |
            //    lshift|---|               |--rshift---|
            //          |e,e|0,0,0,0,0,0,0,0|0,0,0,0,0,0|
            //              |<==   ||    ==>|
            //               bits_to_discard (from these bytes)
            //
            // variable(bits_to_discard): Tracks the amount of bits to be
            // discarded and is updated accordingly in each iteration.
            //
            // It monitors these packed bits from respective word_i and
            // word_i+1 and overwrites them with zeros accordingly.
            // It discards linearly from the lowest index and moves upwards
            // until bits_to_discard variable becomes zero.
            //
            // In the above diagram,for example, we pack 2bytes(7th and 6th
            // from word_i) and 6bytes(0th till 5th from word_i+1), combine
            // and pack them as 64bit to output buffer out[i].
            // Depending on the number of bits requested, we discard the
            // bits from these packed bytes by overwriting them with zeros.
            if bits_to_discard < (BITS_PER_WORD - rshift) {
                // If the bits to be discarded is lesser than the amount of bits
                // copied to the output buffer from word_i, we discard that much
                // amount of bits only.
                self.entropy[pool_idx0] &= u64::MAX << ((bits_to_discard + rshift) % BITS_PER_WORD);
                bits_to_discard = 0;
            } else {
                // If the bits to be discarded is more than the amount of valid
                // upper bits from word_i, which has been copied to the output
                // buffer, we just set the entire word_i to 0, as the lower bits
                // will be already zeros from previous operations, and the
                // bits_to_discard is updated precisely.
                self.entropy[pool_idx0] = 0;
                bits_to_discard -= BITS_PER_WORD - rshift;
            }

            // Note that a shift of 64 bits is treated as a shift of 0 bits.
            // When the shift amount is the same as the BITS_PER_WORD, we
            // don't want to include the next word of entropy, so we skip
            // the `|=` operation.
            if lshift != BITS_PER_WORD {
                let pool_idx1 = (min_word + idx + 1) % WORDS_IN_POOL;
                *word_i |= self.entropy[pool_idx1] << lshift;

                if bits_to_discard < (BITS_PER_WORD - lshift) {
                    // Discarding the remaining packed bits from upperword
                    // (word[i+1]) which was copied to output buffer by
                    // overwriting with zeros.
                    //
                    // If the remaining bits to be discarded is lesser than
                    // the amount of bits from [word_i+1], which has been
                    // copied to the output buffer, we overwrite that much
                    // amount of bits only.
                    self.entropy[pool_idx1] &= u64::MAX << (bits_to_discard % BITS_PER_WORD);
                    bits_to_discard = 0;
                } else {
                    // If bits to discard is more than the bits from word_i+1
                    // which got packed into the output, then we discard all
                    // those copied bits.
                    //
                    // Note: we cannot set the entire word_i+1 to 0, as
                    // there are still some unused valid entropy bits at the
                    // upper end for future use.
                    self.entropy[pool_idx1] &=
                        u64::MAX << ((BITS_PER_WORD - lshift) % BITS_PER_WORD);
                    bits_to_discard -= BITS_PER_WORD - lshift;
                }
            }
        }

        // Mask off higher bits if only part of the last word was requested
        if nbits % BITS_PER_WORD != 0 {
            let mask = u64::MAX >> (BITS_PER_WORD - (nbits % BITS_PER_WORD));
            out[to_fill - 1] &= mask;
        }

        self.entropy_bit_index = (self.entropy_bit_index + nbits) % BITS_IN_POOL;
        self.entropy_bit_size -= nbits;

        Ok(())
    }
}

pub struct Trng {
    pool: SpinMutex<EntropyPool>,
}

impl Service for Trng {
    owns!(
        OwningEntityNumber::STANDARD_SECURE,
        TRNG_FN_NUM_MIN..=TRNG_FN_NUM_MAX
    );

    fn handle_non_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        (self.handle_smc_common(regs), World::NonSecure)
    }

    fn handle_secure_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        (self.handle_smc_common(regs), World::Secure)
    }

    #[cfg(feature = "rme")]
    fn handle_realm_smc(&self, regs: &[u64; 18]) -> (SmcReturn, World) {
        (self.handle_smc_common(regs), World::Realm)
    }
}

impl Trng {
    pub(super) fn new() -> Self {
        TrngPlatformImpl::entropy_setup();
        Self {
            pool: SpinMutex::new(EntropyPool::new()),
        }
    }

    fn handle_smc_common(&self, regs: &[u64; 18]) -> SmcReturn {
        let mut function = FunctionId(regs[0] as u32);
        function.clear_sve_hint();
        let x1 = regs[1];

        if TrngPlatformImpl::TRNG_UUID.is_nil() {
            return TrngError::NotSupported.into();
        }

        match function.0 {
            ARM_TRNG_VERSION => TRNG_VERSION.into(),
            ARM_TRNG_FEATURES => {
                let feature_id = x1 as u32;
                if is_trng_fid(feature_id) {
                    SUCCESS.into()
                } else {
                    TrngError::NotSupported.into()
                }
            }
            ARM_TRNG_GET_UUID => TrngPlatformImpl::TRNG_UUID.into(),
            ARM_TRNG_RND32 => self.trng_rnd32(x1 as usize),
            ARM_TRNG_RND64 => self.trng_rnd64(x1 as usize),
            _ => TrngError::NotSupported.into(),
        }
    }

    /// Generate n bits of entropy for an SMC32 call
    fn trng_rnd32(&self, nbits: usize) -> SmcReturn {
        if nbits == 0 || nbits > TRNG_RND32_ENTROPY_MAXBITS {
            return TrngError::InvalidParams.into();
        }

        let mut ent = [0u64; 2];
        if let Err(e) = self.pool.lock().pack_entropy(nbits, &mut ent) {
            return e.into();
        }

        // Return entropy in w1-w3 as per SMC32 definition in TRNG spec
        [
            SUCCESS as u64,
            ent[1],
            (ent[0] >> 32) & 0xFFFF_FFFF,
            ent[0] & 0xFFFF_FFFF,
        ]
        .into()
    }

    /// Generate n bits of entropy for an SMC64 call
    fn trng_rnd64(&self, nbits: usize) -> SmcReturn {
        if nbits == 0 || nbits > TRNG_RND64_ENTROPY_MAXBITS {
            return TrngError::InvalidParams.into();
        }

        let mut ent = [0u64; 3];
        if let Err(e) = self.pool.lock().pack_entropy(nbits, &mut ent) {
            return e.into();
        }

        // Return entropy in x1-x3 as per SMC64 definition in TRNG spec
        [SUCCESS as u64, ent[2], ent[1], ent[0]].into()
    }
}

fn is_trng_fid(smc_fid: u32) -> bool {
    matches!(
        smc_fid,
        ARM_TRNG_VERSION | ARM_TRNG_FEATURES | ARM_TRNG_GET_UUID | ARM_TRNG_RND32 | ARM_TRNG_RND64
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_entropy_less_than_word() {
        let mut pool = EntropyPool::new();
        let mut out = [0u64; 1];

        let nbits = 23;
        pool.pack_entropy(nbits, &mut out).unwrap();
        assert_eq!(out[0], (1u64 << nbits).wrapping_sub(1));
        assert_eq!(pool.entropy_bit_size, BITS_PER_WORD - nbits);
        assert_eq!(pool.entropy_bit_index, nbits);
    }

    #[test]
    fn pack_entropy_one_word() {
        let mut pool = EntropyPool::new();
        let mut out = [0u64; 2];

        let nbits = 64;
        pool.pack_entropy(nbits, &mut out).unwrap();
        assert_eq!(out[0], u64::MAX);
        assert_eq!(out[1], 0); // Not enough bits for out[1].
        assert_eq!(pool.entropy_bit_size, 0);
        assert_eq!(pool.entropy_bit_index, nbits % BITS_IN_POOL);
    }

    #[test]
    fn pack_entropy_multiple_words() {
        let mut pool = EntropyPool::new();
        let mut out = [0u64; 3];

        let nbits = 192;
        pool.pack_entropy(nbits, &mut out).unwrap();
        assert_eq!(out[0], u64::MAX);
        assert_eq!(out[1], u64::MAX);
        assert_eq!(out[2], u64::MAX);
        assert_eq!(pool.entropy_bit_size, 0);
        assert_eq!(pool.entropy_bit_index, nbits % BITS_IN_POOL);
    }

    #[test]
    fn pack_entropy_unaligned_requests() {
        let mut pool = EntropyPool::new();
        let mut out = [0u64; 1];

        // Request 30 bits first.
        let nbits0 = 30;
        pool.pack_entropy(nbits0, &mut out).unwrap();
        assert_eq!(out[0], (1u64 << nbits0).wrapping_sub(1));
        assert_eq!(pool.entropy_bit_size, BITS_PER_WORD - nbits0);
        assert_eq!(pool.entropy_bit_index, nbits0);

        // Request another 50 bits.
        out[0] = 0;
        let nbits1 = 50;
        pool.pack_entropy(nbits1, &mut out).unwrap();
        assert_eq!(out[0], (1u64 << nbits1).wrapping_sub(1));
        assert_eq!(pool.entropy_bit_size, BITS_PER_WORD * 2 - nbits0 - nbits1);
        assert_eq!(pool.entropy_bit_index, (nbits0 + nbits1) % BITS_IN_POOL);
    }

    #[test]
    fn pack_entropy_wraps_around_pool() {
        let mut pool = EntropyPool {
            entropy: [u64::MAX; WORDS_IN_POOL],
            entropy_bit_index: BITS_IN_POOL - 32, // Start 32 bits from the end
            entropy_bit_size: BITS_IN_POOL,
        };

        let mut out = [0u64; 2];
        let nbits = 64;
        // This request will take 32 bits from the last word and 32 from the first.
        pool.pack_entropy(nbits, &mut out).unwrap();

        assert_eq!(out[0], u64::MAX);
        assert_eq!(
            pool.entropy_bit_index,
            (BITS_IN_POOL - 32 + nbits) % BITS_IN_POOL
        );
        assert_eq!(pool.entropy_bit_size, BITS_IN_POOL - nbits);
    }

    #[test]
    fn pack_entropy_all_bits() {
        let mut pool = EntropyPool::new();
        let mut out = [0u64; 4];

        let nbits = BITS_IN_POOL;
        pool.pack_entropy(nbits, &mut out).unwrap();
        assert_eq!(out[0], u64::MAX);
        assert_eq!(out[1], u64::MAX);
        assert_eq!(out[2], u64::MAX);
        assert_eq!(out[3], u64::MAX);
        assert_eq!(pool.entropy_bit_size, 0);
        assert_eq!(pool.entropy_bit_index, 0);
    }

    #[test]
    fn trng_version() {
        let trng = Trng::new();
        let regs = [
            ARM_TRNG_VERSION as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TRNG_VERSION.into());
    }

    #[test]
    fn trng_features() {
        let trng = Trng::new();
        // Supported feature
        let regs = [
            ARM_TRNG_FEATURES as u64,
            ARM_TRNG_RND32 as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, SUCCESS.into());

        // Unsupported feature
        let regs = [
            ARM_TRNG_FEATURES as u64,
            0x8400_0000_u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TrngError::NotSupported.into());
    }

    #[test]
    fn trng_get_uuid() {
        let trng = Trng::new();
        let regs = [
            ARM_TRNG_GET_UUID as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        let actual_uuid = Uuid::from_u128_le(
            ret.values()[0] as u128
                | ((ret.values()[1] as u128) << 32)
                | ((ret.values()[2] as u128) << 64)
                | ((ret.values()[3] as u128) << 96),
        );
        let expected_uuid = TrngPlatformImpl::TRNG_UUID;
        assert_eq!(actual_uuid, expected_uuid);
    }

    #[test]
    fn trng_rnd32_invalid_nbits() {
        let trng = Trng::new();
        // nbits == 0
        let regs = [
            ARM_TRNG_RND32 as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TrngError::InvalidParams.into());

        // nbits > max
        let regs = [
            ARM_TRNG_RND32 as u64,
            (TRNG_RND32_ENTROPY_MAXBITS + 1) as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TrngError::InvalidParams.into());
    }

    #[test]
    fn trng_rnd64_invalid_nbits() {
        let trng = Trng::new();
        // nbits = 0
        let regs = [
            ARM_TRNG_RND64 as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TrngError::InvalidParams.into());

        // nbits > max
        let regs = [
            ARM_TRNG_RND64 as u64,
            (TRNG_RND64_ENTROPY_MAXBITS + 1) as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        assert_eq!(ret, TrngError::InvalidParams.into());
    }

    #[test]
    fn trng_rnd32_get_entropy() {
        let trng = Trng::new();
        let nbits = 12;
        let regs = [
            ARM_TRNG_RND32 as u64,
            nbits,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        let expected_entropy = (1u64 << nbits).wrapping_sub(1);
        let expected: SmcReturn = [SUCCESS as u64, 0, 0, expected_entropy].into();
        assert_eq!(ret, expected);

        let regs = [
            ARM_TRNG_RND32 as u64,
            TRNG_RND32_ENTROPY_MAXBITS as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        let expected: SmcReturn = [
            SUCCESS as u64,
            u32::MAX as u64,
            u32::MAX as u64,
            u32::MAX as u64,
        ]
        .into();
        assert_eq!(ret, expected);
    }

    #[test]
    fn trng_rnd64_get_entropy() {
        let trng = Trng::new();
        let nbits = 51;
        let regs = [
            ARM_TRNG_RND64 as u64,
            nbits,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        let expected_entropy: u64 = (1u64 << nbits).wrapping_sub(1);
        let expected: SmcReturn = [SUCCESS as u64, 0, 0, expected_entropy].into();
        assert_eq!(ret, expected);

        let regs = [
            ARM_TRNG_RND64 as u64,
            TRNG_RND64_ENTROPY_MAXBITS as u64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ];
        let ret = trng.handle_smc_common(&regs);
        let expected: SmcReturn = [SUCCESS as u64, u64::MAX, u64::MAX, u64::MAX].into();
        assert_eq!(ret, expected);
    }
}
