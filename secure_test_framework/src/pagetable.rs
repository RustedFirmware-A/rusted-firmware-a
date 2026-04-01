// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use aarch64_paging::descriptor::El23Attributes;

pub const DEVICE_ATTRIBUTES: El23Attributes = El23Attributes::VALID
    .union(El23Attributes::ATTRIBUTE_INDEX_0)
    .union(El23Attributes::ACCESSED)
    .union(El23Attributes::XN);
pub const MEMORY_ATTRIBUTES: El23Attributes = El23Attributes::VALID
    .union(El23Attributes::ATTRIBUTE_INDEX_1)
    .union(El23Attributes::INNER_SHAREABLE)
    .union(El23Attributes::ACCESSED)
    .union(El23Attributes::NON_GLOBAL);
