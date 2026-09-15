// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! MHU based doorbell implementation for shared memory SCMI transport.

use arm_mhu::doorbell::MhuPostboxDoorbell;
use arm_scmi::transport::shared_memory::Doorbell;

/// MHU based doorbell implementation for shared memory SCMI transport.
pub struct MhuScmiDoorbell<'a> {
    doorbell: MhuPostboxDoorbell<'a>,
    flags: u32,
}

impl<'a> MhuScmiDoorbell<'a> {
    /// Creates a new instance that sets `flags` when ringing the doorbell.
    pub fn new(doorbell: MhuPostboxDoorbell<'a>, flags: u32) -> Self {
        Self { doorbell, flags }
    }
}

impl<'a> Doorbell for MhuScmiDoorbell<'a> {
    fn ring(&mut self) {
        self.doorbell.set_flags(self.flags);
    }
}
