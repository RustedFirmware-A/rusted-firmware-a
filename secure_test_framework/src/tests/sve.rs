// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests SVE context switch when Secure World uses only SIMD.

use crate::{
    framework::{TestHelperProxy, TestResult, normal_world_test},
    tests::simd::util::{
        Phase, READ_BUFFER_PREDICATES, READ_BUFFER_VECTORS, SvePredicates, SveVectors,
        get_vl_bytes, is_sve_present, overwrite_sve_predicates, overwrite_sve_vectors,
        read_sve_predicates, read_sve_vectors, test_simd_context_switch_helper,
    },
};

use log::debug;
use spin::mutex::SpinMutex;

// Use static variables for NSWd SVE state to avoid stack overflow.
static NSWD_SVE_VECTORS: SpinMutex<SveVectors> = SpinMutex::new([0; 32 * 16]);
static NSWD_SVE_PREDICATES: SpinMutex<SvePredicates> = SpinMutex::new([0; 32]);

normal_world_test!(
    test_sve_context_switch,
    helper = test_simd_context_switch_helper
);

/// Checks if NSWd SVE vector registers' state is preserved across world switches
/// when SWd uses regular SIMD.
fn test_sve_context_switch(helper: &TestHelperProxy) -> TestResult {
    if !is_sve_present() {
        debug!("SVE not present, skipping test.");
        return Ok(());
    }

    debug!("NSWd SVE context switch test starting...");
    let vl_bytes = get_vl_bytes();
    debug!(
        "SVE Vector Length: {} bits ({} bytes)",
        vl_bytes * 8,
        vl_bytes
    );

    let vl_u128 = vl_bytes / 16;
    let pl_u128 = vl_bytes / 128; // 1/8 of VL

    {
        let mut nswd_vectors = NSWD_SVE_VECTORS.lock();
        let mut nswd_predicates = NSWD_SVE_PREDICATES.lock();

        // NSWd overwrites SVE vector registers with unique values.
        // We write to both the low 128 bits (aliased with SIMD) and higher bits.
        for i in 0..32 {
            // Low 128 bits
            nswd_vectors[i * vl_u128] = 0x1111_2222_3333_4444_5555_6666_7777_8888
                ^ (i as u128).wrapping_mul(0x2d3a_5b6c_7e8f_9a0b_1c2d_3e4f_5a6b_7c8d);
            // If VL > 128, write to higher bits too.
            if vl_u128 > 1 {
                nswd_vectors[i * vl_u128 + 1] = 0xAAAA_BBBB_CCCC_DDDD_EEEE_FFFF_0000_1111
                    ^ (i as u128).wrapping_mul(0x4f5e_6d7c_8b9a_0b1c_2d3e_4f5a_6b7c_8d9e);
            }
        }
        // NSWd overwrites SVE predicate registers.
        for i in 0..16 {
            nswd_predicates[i * pl_u128] = 0x55AA_55AA_55AA_55AA_55AA_55AA_55AA_55AA
                ^ (i as u128).wrapping_mul(0x1a2b_3c4d_5e6f_7a8b_9c0d_1e2f_3a4b_5c6d);
        }

        overwrite_sve_vectors(&nswd_vectors);
        overwrite_sve_predicates(&nswd_predicates);

        let mut read_vecs = READ_BUFFER_VECTORS.lock();
        read_sve_vectors(&mut read_vecs);
        for i in 0..32 {
            assert_eq!(
                read_vecs[i * vl_u128],
                nswd_vectors[i * vl_u128],
                "NSWd failed to overwrite SVE register Z{i} (low)"
            );
            if vl_u128 > 1 {
                assert_eq!(
                    read_vecs[i * vl_u128 + 1],
                    nswd_vectors[i * vl_u128 + 1],
                    "NSWd failed to overwrite SVE register Z{i} (high)"
                );
            }
        }
        let mut read_preds = READ_BUFFER_PREDICATES.lock();
        read_sve_predicates(&mut read_preds);
        for i in 0..16 {
            assert_eq!(
                read_preds[i * pl_u128],
                nswd_predicates[i * pl_u128],
                "NSWd failed to overwrite SVE register P{i}"
            );
        }
        debug!("NSWd SVE registers initialized and verified");
    }

    // Switch to the SWd side of the test.
    debug!("Switching to SWd to overwrite its SIMD state");
    helper(Phase::SWdOverwriteSIMD.into())?;

    // Make sure the world switch did not destroy NSWd SVE state.
    debug!("Checking NSWd SVE state after world switch");
    {
        let nswd_vectors = NSWD_SVE_VECTORS.lock();
        let nswd_predicates = NSWD_SVE_PREDICATES.lock();

        let mut read_vecs = READ_BUFFER_VECTORS.lock();
        read_sve_vectors(&mut read_vecs);
        for i in 0..32 {
            assert_eq!(
                read_vecs[i * vl_u128],
                nswd_vectors[i * vl_u128],
                "NSWd SVE Z{i} (low) should be preserved across world switch"
            );
            if vl_u128 > 1 {
                assert_eq!(
                    read_vecs[i * vl_u128 + 1],
                    nswd_vectors[i * vl_u128 + 1],
                    "NSWd SVE Z{i} (high) should be preserved across world switch"
                );
            }
        }
        let mut read_preds = READ_BUFFER_PREDICATES.lock();
        read_sve_predicates(&mut read_preds);
        for i in 0..16 {
            assert_eq!(
                read_preds[i * pl_u128],
                nswd_predicates[i * pl_u128],
                "NSWd SVE P{i} should be preserved across world switch"
            );
        }
        debug!("NSWd SVE state preserved successfully");
    }

    debug!("SVE context switch test passed");
    Ok(())
}
