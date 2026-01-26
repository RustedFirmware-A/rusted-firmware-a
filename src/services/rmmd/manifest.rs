// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use core::fmt::{Debug, Display};

use aarch64_paging::paging::PAGE_SIZE;
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::services::rmmd::RMM_SHARED_BUFFER_SIZE;

pub struct RmmBootManifest<'a> {
    pub version: RmmBootManifestVersion,
    pub plat_data: &'a [u8],
    pub plat_dram: &'a [RmmMemoryBank],
    pub plat_console: &'a [RmmConsoleInfo],
    pub plat_ncoh_region: &'a [RmmMemoryBank],
    pub plat_coh_region: &'a [RmmMemoryBank],
    pub plat_smmu: &'a [RmmSmmuInfo],
    pub plat_root_complex: RmmRootComplexInfoList<'a>,
}

impl<'a> RmmBootManifest<'a> {
    /// Writes the boot manifest in the given memory region.
    ///
    /// The buffer provided must be page-aligned and `buf_pa` must be its physical address.
    pub fn pack(&self, buf: &mut [u8; RMM_SHARED_BUFFER_SIZE], buf_pa: usize) {
        assert!((buf.as_ptr() as usize).is_multiple_of(PAGE_SIZE));

        macro_rules! make_list {
            ($self:expr, $hdr:ident, $list:ident, $buf:ident, $addr_of:expr) => {
                let self_list = $self.$list.as_bytes();
                let (hdr_list, $buf) = $buf.split_at_mut(self_list.len());
                $hdr.$list.num_entries = $self.$list.len() as _;
                $hdr.$list.entries_ptr = if $self.$list.is_empty() {
                    0
                } else {
                    $addr_of(hdr_list) as u64
                };
                $hdr.$list.checksum = Self::compute_checksum(
                    self_list,
                    &[$hdr.$list.num_entries, $hdr.$list.entries_ptr],
                );
                hdr_list.copy_from_slice(self_list);
            };
        }

        let buf_ptr = buf.as_ptr() as usize;
        let addr_of = |b: &[u8]| {
            let off = b.as_ptr() as usize - buf_ptr;
            assert!(off < RMM_SHARED_BUFFER_SIZE);
            buf_pa + off
        };

        let (hdr, buf) = RmmBootManifestHeader::mut_from_prefix(buf).unwrap();

        hdr.version = self.version.into();

        let (plat_data, buf) = buf.split_at_mut(self.plat_data.len());
        hdr.plat_data = addr_of(plat_data);
        plat_data.copy_from_slice(self.plat_data);

        make_list!(self, hdr, plat_dram, buf, addr_of);
        make_list!(self, hdr, plat_console, buf, addr_of);
        make_list!(self, hdr, plat_ncoh_region, buf, addr_of);
        make_list!(self, hdr, plat_coh_region, buf, addr_of);
        make_list!(self, hdr, plat_smmu, buf, addr_of);

        let mut rem_buf;
        let hdr_root_buf;

        (hdr_root_buf, rem_buf) = buf.split_at_mut(
            self.plat_root_complex.entries.len() * size_of::<RmmRootComplexInfoInternal>(),
        );

        hdr.plat_root_complex.rc_info_version = self.plat_root_complex.rc_info_version.into();
        hdr.plat_root_complex.padding = 0;
        hdr.plat_root_complex.entries_ptr = addr_of(hdr_root_buf) as _;
        hdr.plat_root_complex.num_entries = self.plat_root_complex.entries.len() as _;

        let hdr_roots_list = <[RmmRootComplexInfoInternal]>::mut_from_bytes(hdr_root_buf).unwrap();

        for (hdr_root, self_root) in hdr_roots_list
            .iter_mut()
            .zip(self.plat_root_complex.entries)
        {
            let hdr_ports_buf;
            (hdr_ports_buf, rem_buf) = rem_buf
                .split_at_mut(self_root.entries.len() * size_of::<RmmRootPortInfoInternal>());

            hdr_root.padding = [0; 3];
            hdr_root.ecam_base = self_root.ecam_base;
            hdr_root.segment = self_root.segment;
            hdr_root.num_entries = self_root.entries.len() as _;
            hdr_root.entries_ptr = addr_of(hdr_ports_buf) as _;

            let hdr_ports_list =
                <[RmmRootPortInfoInternal]>::mut_from_bytes(hdr_ports_buf).unwrap();
            for (hdr_port, self_port) in hdr_ports_list.iter_mut().zip(self_root.entries) {
                let hdr_bdfs_buf;
                (hdr_bdfs_buf, rem_buf) =
                    rem_buf.split_at_mut(core::mem::size_of_val(self_port.entries));

                hdr_port.root_port_id = self_port.root_port_id;
                hdr_port.num_entries = self_port.entries.len() as _;
                hdr_port.entries_ptr = addr_of(hdr_bdfs_buf) as _;

                let hdr_bdfs_list = <[BdfMappingInfo]>::mut_from_bytes(hdr_bdfs_buf).unwrap();
                hdr_bdfs_list.copy_from_slice(self_port.entries);
            }
        }

        let rem_len = rem_buf.len();
        let root_complex_len = buf.len() - rem_len;

        hdr.plat_root_complex.checksum = Self::compute_checksum(
            &buf[..root_complex_len],
            &[
                hdr.plat_root_complex.num_entries,
                hdr.plat_root_complex.rc_info_version as u64,
                hdr.plat_root_complex.entries_ptr,
            ],
        );
    }

    fn compute_checksum(buf: &[u8], other: &[u64]) -> u64 {
        assert!(buf.len().is_multiple_of(size_of::<u64>() / size_of::<u8>()));

        let buf = <[u64]>::ref_from_bytes(buf).unwrap();
        buf.iter().chain(other).sum::<u64>().wrapping_neg()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// The RMM-EL3 Boot Manifest v0.5 structure contains platform boot information passed from EL3 to
/// RMM.
struct RmmBootManifestHeader {
    /// Boot Manifest version.
    version: u32,
    /// Reserved, set to 0.
    padding: [u8; 4],
    /// Pointer to Platform Data section.
    plat_data: usize,
    /// NS DRAM Layout Info structure.
    plat_dram: ChecksummedList,
    /// List of consoles available to RMM.
    plat_console: ChecksummedList,
    /// Device non-coherent ranges Info structure.
    plat_ncoh_region: ChecksummedList,
    /// Device coherent ranges Info structure.
    plat_coh_region: ChecksummedList,
    /// List of SMMUs available to RMM (from Boot Manifest v0.5).
    plat_smmu: ChecksummedList,
    /// List of PCIe root complexes available to RMM (from Boot Manifest v0.5).
    plat_root_complex: RmmRootComplexListInternal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Immutable, FromBytes, IntoBytes)]
pub struct RmmBootManifestVersion {
    pub(crate) minor: u16,
    pub(crate) major: u16,
}

impl TryFrom<u32> for RmmBootManifestVersion {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        let major = (value >> 16) as u16;

        if major & 0x7fff != major {
            return Err(());
        }

        Ok(Self {
            major,
            minor: (value & 0xFFFF) as u16,
        })
    }
}

impl From<RmmBootManifestVersion> for u32 {
    fn from(value: RmmBootManifestVersion) -> Self {
        (value.major as u32) << 16 | value.minor as u32
    }
}

impl Display for RmmBootManifestVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("v{}.{}", self.major, self.minor))
    }
}

/// Current version of the `RmmBootManifest` struct.
pub const RMM_BOOT_MANIFEST_VERSION: RmmBootManifestVersion =
    RmmBootManifestVersion { major: 0, minor: 5 };
pub const RMM_BOOT_MANIFEST_ROOT_COMPLEX_VERSION: RmmBootManifestVersion =
    RmmBootManifestVersion { major: 0, minor: 1 };

#[derive(Debug, Copy, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
struct ChecksummedList {
    num_entries: u64,
    entries_ptr: u64,
    checksum: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// Memory Bank structure contains information about each memory bank/device region.
pub struct RmmMemoryBank {
    /// Base address.
    pub base: usize,
    /// Size of memory bank/device region in bytes.
    pub size: usize,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// Console Info structure contains information about each Console available to RMM.
pub struct RmmConsoleInfo {
    /// Console Base address.
    pub base: usize,
    /// Num of pages to map for console MMIO.
    pub map_pages: usize,
    /// Name of console.
    pub name: [u8; 8],
    /// UART clock (in Hz) for console.
    pub clk_in_hz: u64,
    /// Baud rate.
    pub baud_rate: u64,
    /// Additional flags (reserved, MBZ).
    pub flags: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// SMMU Info structure contains information about each SMMU available to RMM.
pub struct RmmSmmuInfo {
    /// SMMU Base address.
    pub smmu_base: usize,
    /// SMMU Realm Pages base address.
    pub smmu_r_base: usize,
}

pub struct RmmRootComplexInfoList<'a> {
    pub rc_info_version: RmmBootManifestVersion,
    pub entries: &'a [RmmRootComplexInfo<'a>],
}

pub struct RmmRootComplexInfo<'a> {
    pub ecam_base: u64,
    pub segment: u8,
    pub entries: &'a [RmmRootPortInfo<'a>],
}

#[derive(Debug, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
struct RmmRootComplexListInternal {
    num_entries: u64,
    /// Root Complex Info structure version.
    rc_info_version: u32,
    /// Reserved, set to 0.
    padding: u32,
    entries_ptr: u64,
    checksum: u64,
}

#[derive(Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// Root Complex Info structure contains information about each PCIe root complex available to RMM.
struct RmmRootComplexInfoInternal {
    /// PCIe ECAM Base address.
    ecam_base: u64,
    /// PCIe segment identifier.
    segment: u8,
    /// Reserved, set to 0.
    padding: [u8; 3],
    num_entries: u32,
    entries_ptr: u64,
}

/// Root Complex Info structure contains information about each root port in PCIe root complex.
pub struct RmmRootPortInfo<'a> {
    /// Root Port identifier.
    pub root_port_id: u16,
    pub entries: &'a [BdfMappingInfo],
}

#[derive(Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// Root Complex Info structure contains information about each root port in PCIe root complex.
struct RmmRootPortInfoInternal {
    /// Root Port identifier.
    root_port_id: u16,
    /// Reserved, set to 0.
    padding: u16,
    num_entries: u32,
    entries_ptr: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Immutable, FromBytes, IntoBytes, KnownLayout)]
#[repr(C)]
/// BDF Mapping Info structure contains information about each Device-Bus-Function (BDF) mapping for
/// PCIe root port.
pub struct BdfMappingInfo {
    /// Base of BDF mapping (inclusive).
    pub mapping_base: u16,
    /// Top of BDF mapping (exclusive).
    pub mapping_top: u16,
    /// Mapping offset, as per Arm Base System Architecture:
    /// StreamID = RequesterID[N-1:0] + (1<<N)*Constant_B.
    pub mapping_off: u16,
    /// SMMU index in [`plat_smmu`][RmmBootManifest::plat_smmu] array.
    pub smmu_idx: u16,
}
