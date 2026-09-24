// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! System ID peripheral driver
//!
//! The implementation is based on
//! <https://support.arm.com/documentation/108028/0000/Programmers-model/Register-descriptions/System-ID-registers>.

use safe_mmio::{UniqueMmioPointer, field_shared, fields::ReadPure};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

/// Register containing version information for the subsystem.
#[derive(Clone, PartialEq, Eq, FromBytes, Immutable, IntoBytes, KnownLayout)]
#[repr(transparent)]
pub struct IdRegister(pub u32);

impl IdRegister {
    const MAJOR_REVISION_MASK: u32 = 0b1111;
    const MAJOR_REVISION_SHIFT: u32 = 24;
    const MINOR_REVISION_MASK: u32 = 0b1111;
    const MINOR_REVISION_SHIFT: u32 = 20;
    const DESIGNER_ID_MASK: u32 = 0b1111_1111;
    const DESIGNER_ID_SHIFT: u32 = 12;
    const PART_NUMBER_MASK: u32 = 0b1111_1111_1111;
    const PART_NUMBER_SHIFT: u32 = 0;

    /// Returns the major revision number for the subsystem.
    pub fn major_revision(&self) -> u8 {
        ((self.0 >> Self::MAJOR_REVISION_SHIFT) & Self::MAJOR_REVISION_MASK) as u8
    }

    /// Returns the minor revision number for the subsystem.
    pub fn minor_revision(&self) -> u8 {
        ((self.0 >> Self::MINOR_REVISION_SHIFT) & Self::MINOR_REVISION_MASK) as u8
    }

    /// Returns the identification code for the subsystem designer.
    pub fn designer_id(&self) -> u8 {
        ((self.0 >> Self::DESIGNER_ID_SHIFT) & Self::DESIGNER_ID_MASK) as u8
    }

    /// Returns the part number for the subsystem.
    pub fn part_number(&self) -> u16 {
        ((self.0 >> Self::PART_NUMBER_SHIFT) & Self::PART_NUMBER_MASK) as u16
    }
}

/// Register containing information about the node when there are multiple sockets.
#[derive(Clone, PartialEq, Eq, FromBytes, Immutable, IntoBytes, KnownLayout)]
#[repr(transparent)]
pub struct NodeIdRegister(pub u32);

impl NodeIdRegister {
    const MULTICHIP_BIT: u32 = 1 << 8;
    const NODE_NUMBER_MASK: u32 = 0b1111_1111;
    const NODE_NUMBER_SHIFT: u32 = 0;

    /// Returns the node number or `None` on a single chip system.
    pub fn node_number(&self) -> Option<u8> {
        if self.0 & Self::MULTICHIP_BIT != 0 {
            Some(((self.0 >> Self::NODE_NUMBER_SHIFT) & Self::NODE_NUMBER_MASK) as u8)
        } else {
            None
        }
    }
}

/// System ID registers
#[derive(Clone, PartialEq, Eq, FromBytes, Immutable, IntoBytes, KnownLayout)]
#[repr(C, align(4))]
pub struct SystemIdRegisters {
    reserved0: [u32; 16],
    /// 0x040 ID register
    sid_system_id: ReadPure<IdRegister>,
    reserved44: [u32; 3],
    /// 0x050 ID register
    sid_soc_id: ReadPure<IdRegister>,
    reserved54: [u32; 3],
    /// 0x060 ID register
    sid_node_id: ReadPure<NodeIdRegister>,
    reserved64: [u32; 3],
    /// 0x070 Configuration register
    sid_system_cfg: ReadPure<u32>,
    reserved_4c: [u32; 983],
    /// 0xFD0: SID Peripheral ID 4 register
    pid4: ReadPure<u32>,
    reserved_fd4: [u32; 3],
    /// 0xFE0: SID Peripheral ID 0 register
    pid0: ReadPure<u32>,
    /// 0xFE4: SID Peripheral ID 1 register
    pid1: ReadPure<u32>,
    /// 0xFE8: SID Peripheral ID 2 register
    pid2: ReadPure<u32>,
    /// 0xFEC: SID Peripheral ID 3 register
    pid3: ReadPure<u32>,
    /// 0xFF0: SID Component ID 0 register
    cid0: ReadPure<u32>,
    /// 0xFF4: SID Component ID 1 register
    cid1: ReadPure<u32>,
    /// 0xFF8: SID Component ID 2 register
    cid2: ReadPure<u32>,
    /// 0xFFC: SID Component ID 3 register
    cid3: ReadPure<u32>,
}

/// System ID peripheral driver.
pub struct SystemId<'a> {
    regs: UniqueMmioPointer<'a, SystemIdRegisters>,
}

impl<'a> SystemId<'a> {
    /// Creates a new instance.
    pub fn new(regs: UniqueMmioPointer<'a, SystemIdRegisters>) -> Self {
        Self { regs }
    }

    /// Returns the config number.
    pub fn config_number(&self) -> u32 {
        field_shared!(self.regs, sid_system_cfg).read()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::offset_of;

    #[test]
    fn size_and_offsets() {
        assert_eq!(size_of::<SystemIdRegisters>(), 0x1000);
        assert_eq!(offset_of!(SystemIdRegisters, sid_system_id), 0x40);
        assert_eq!(offset_of!(SystemIdRegisters, sid_soc_id), 0x50);
        assert_eq!(offset_of!(SystemIdRegisters, sid_node_id), 0x60);
        assert_eq!(offset_of!(SystemIdRegisters, sid_system_cfg), 0x70);
        assert_eq!(offset_of!(SystemIdRegisters, pid4), 0xfd0);
        assert_eq!(offset_of!(SystemIdRegisters, pid0), 0xfe0);
    }
}
