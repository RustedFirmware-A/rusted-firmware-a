// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Common components specific to Arm platforms.

#![cfg_attr(not(any(test, feature = "fakes")), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub mod power_management;
pub mod reexports;
