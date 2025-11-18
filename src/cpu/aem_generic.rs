// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use super::Cpu;
use crate::{
    aarch64::{dsb_sy, isb},
    naked_asm,
};
use arm_sysregs::{
    CacheLevel, CacheType, CsselrEl1, MidrEl1, read_ccsidr_el1, read_clidr_el1,
    read_id_aa64mmfr2_el1, write_csselr_el1,
};
use core::{arch::asm, ops::RangeInclusive};

pub struct AemGeneric;

/// The cache size descriptor.
#[derive(Debug)]
struct CacheSize {
    /// (Number of sets in cache) - 1, therefore a value of 0 indicates 1 set in the cache. The
    /// number of sets does not have to be a power of 2.
    pub num_sets: u32,
    /// (Associativity of cache) - 1, therefore a value of 0 indicates an associativity of 1. The
    /// associativity does not have to be a power of 2.
    pub associativity: u32,
    /// (Log2(Number of bytes in cache line)) - 4
    pub line_size_log: u32,
}

/// Reads the cache size description of the given level of cache.
fn read_cache_size(level: CacheLevel) -> CacheSize {
    // Select cache level
    write_csselr_el1(CsselrEl1::new(false, level, false));
    isb();

    // Read and extract the fields of CCSIDR_EL1 according to the presence of CCIDX feature.
    let ccsidr = read_ccsidr_el1();

    let (num_sets, associativity) = if read_id_aa64mmfr2_el1().has_64_bit_ccsidr_el1() {
        (
            ((ccsidr >> 32) & 0x00ff_ffff) as u32,
            ((ccsidr >> 3) & 0x001f_ffff) as u32,
        )
    } else {
        (
            ((ccsidr >> 13) & 0x0000_7fff) as u32,
            ((ccsidr >> 3) & 0x0000_03ff) as u32,
        )
    };

    let line_size_log = (ccsidr & 0b111) as u32;

    CacheSize {
        num_sets,
        associativity,
        line_size_log,
    }
}

/// Flushes a range of cache levels.
///
/// # Safety
///
/// Calling this function between a LoadExcl/StoreExcl sequence is not permitted, as doing cache
/// maintenance operations during this window can lead to unpredictable behavior (see 'B2.12.5
/// Load-Exclusive and Store-Exclusive Instruction Usage Restrictions'). The only exception is
/// when the function is restricted to architectures that guarantee predictable behavior for
/// exclusive accesses.
unsafe fn flush_cache_levels(levels: RangeInclusive<u8>) {
    let clidr_el1 = read_clidr_el1();

    for level_num in levels {
        let level = CacheLevel::new(level_num);

        // Check if the cache level is implemented.
        match clidr_el1.ctype(level) {
            CacheType::NoCache | CacheType::InstructionOnly => continue,
            _ => {}
        };

        // Read cache size and calculate way and set iteration variables.
        let cache_size = read_cache_size(level);

        let assoc_shift = cache_size.associativity.leading_zeros() as u64;
        let ways_aligned = u64::from(cache_size.associativity << assoc_shift);
        let way_step = 1 << assoc_shift;
        let line_length = 1 << (cache_size.line_size_log + 4);
        let max_set_num = u64::from(cache_size.num_sets << (cache_size.line_size_log + 4));

        dsb_sy();

        #[allow(unused)]
        for way in (0..=ways_aligned).step_by(way_step) {
            let value = (u64::from(level) << 1) | way;

            for set in (0..=max_set_num).step_by(line_length) {
                // Safety: The inline assembly invokes the 'Data or unified Cache line Clean and Invalidate by Set/Way'
                // instruction using an argument that is based on the reported cache configuration.
                // The caller ensures that the function is not called in an exclusive access window, or it is safe to do
                // so on the given architecture.
                #[cfg(target_arch = "aarch64")]
                unsafe {
                    asm!("dc cisw, {}", options(nostack), in(reg) value | set);
                }
            }
        }
    }

    write_csselr_el1(CsselrEl1::empty());
    dsb_sy();
    isb();
}

/// The AEM FVP requires cache maintenance operations on CPU/Cluster power down, because it does not
/// implement DynamIQ Shared Unit (DSU).
///
/// SAFETY: `reset_handler` and `dump_registers` are implemented as naked functions and don't touch
/// any registers.
unsafe impl Cpu for AemGeneric {
    const MIDR: MidrEl1 = MidrEl1::from_bits_retain(0x410f_d0f0);

    #[unsafe(naked)]
    extern "C" fn reset_handler() {
        naked_asm!("ret");
    }

    #[unsafe(naked)]
    extern "C" fn dump_registers() {
        naked_asm!("ret");
    }

    /// Flushes level 1 and also flushes level 2 cache if level 3 cache is present.
    fn power_down_level0() {
        // Safety: The exclusive memory accesses are predictable with AEM FVP's cache
        // implementation.
        unsafe {
            flush_cache_levels(1..=1);

            if read_clidr_el1().ctype(CacheLevel::new(3)) != CacheType::NoCache {
                flush_cache_levels(2..=2);
            }
        }
    }

    /// Flushes level 1, level 2 and level 3 caches if present.
    fn power_down_level1() {
        // Safety: The exclusive memory accesses are predictable with AEM FVP's cache
        // implementation.
        unsafe { flush_cache_levels(1..=3) }
    }
}
