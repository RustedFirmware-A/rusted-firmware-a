// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests AMU context preservation across world switches.

use crate::{
    expect,
    framework::{
        TestError, TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult,
        normal_world_test,
    },
};
use arm_sysregs::{
    el0::accessors::{
        read_amevcntr00_el0, read_amevcntr01_el0, read_amevcntr02_el0, read_amevcntr03_el0,
    },
    el1::accessors::read_id_aa64pfr0_el1,
};
use log::debug;

normal_world_test!(test_amu_world_switch, helper = swd_helper);

fn test_amu_world_switch(helper: &TestHelperProxy) -> TestResult {
    if !read_id_aa64pfr0_el1().is_feat_amuv1_present() {
        debug!("FEAT_AMUv1 not present, skipping test.");
        return Err(TestError::Ignored);
    }

    let before = read_group0_counters();

    helper(TestHelperRequest::default())?;

    let after = read_group0_counters();

    for (before, after) in before.into_iter().zip(after) {
        expect!(before <= after);
    }

    Ok(())
}

fn swd_helper(_request: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    Ok(TestHelperResponse::default())
}

fn read_group0_counters() -> [u64; 4] {
    [
        read_amevcntr00_el0().acnt(),
        read_amevcntr01_el0().acnt(),
        read_amevcntr02_el0().acnt(),
        read_amevcntr03_el0().acnt(),
    ]
}
