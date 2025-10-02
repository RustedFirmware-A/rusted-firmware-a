// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_sysregs::read_id_aa64mmfr1_el1;

pub fn is_feat_vhe_present() -> bool {
    const VHE: u64 = 1 << 8;

    read_id_aa64mmfr1_el1() & VHE != 0
}
