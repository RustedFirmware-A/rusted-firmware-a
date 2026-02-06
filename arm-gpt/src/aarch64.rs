// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use arm_sysregs::{
    Cacheability, GpccrEl3, GptbrEl3, Shareability, read_gpccr_el3, read_gptbr_el3,
    write_gpccr_el3, write_gptbr_el3,
};
use core::arch::asm;

use crate::{
    Error, GranuleProtection, GranuleProtectionConfig, Level0GptSize, Level0Table,
    PhysicalGranuleSize, ProtectedPhysicalAddressSize, mask,
};

fn isb() {
    // Safety: `isb` does not violate Rust safety.
    unsafe { asm!("isb") }
}
fn dsbsy() {
    // Safety: `isb` does not violate Rust safety.
    unsafe { asm!("dsb sy") }
}
fn tlbi_paallos() {
    // Safety: TLB/Cache invalidation does not violate Rust safety.
    unsafe { asm!("sys #6, c8, c1, #4") }
}

impl GranuleProtection<'static> {
    /// Reads the values from the `GPCCR_EL3` and `GPTBR_EL3` register to locate an existing Granule
    /// Protection Table.
    ///
    /// GPT initialization typically happens in a bootloader stage prior to setting up the EL3
    /// runtime environment for the granule transition service so this function detects the
    /// initialization from a previous stage. Granule protection checks must be enabled already or
    /// this function will return an error.
    ///
    /// # Safety
    ///
    /// This function cannot be called multiple times, unless the [`GranuleProtection`] object
    /// returned by the previous call was dropped.
    pub unsafe fn discover() -> Result<Self, Error> {
        let gpcc = read_gpccr_el3();
        let gptbr = read_gptbr_el3();

        if !gpcc.contains(GpccrEl3::GPC) {
            return Err(Error::GptNotInitialized);
        }

        let config = GranuleProtectionConfig {
            pps: ProtectedPhysicalAddressSize::try_from(gpcc.pps())
                .map_err(|_| Error::InvalidConfiguration)?,
            l0gptsz: Level0GptSize::try_from(gpcc.l0gptsz())
                .map_err(|_| Error::InvalidConfiguration)?,
            pgs: PhysicalGranuleSize::try_from(gpcc.pgs())
                .map_err(|_| Error::InvalidConfiguration)?,
        };

        // Safety: since Granule Protection Checks are enabled, it is safe to assume the the
        // registers are correctly programmed hence GPTBR_EL3 contains the address of a Level0Table
        // whose size is given by the GPCCR_EL3.PPS and GPCCR_EL3.L0GPTSZ fields.
        let level0 = unsafe {
            use core::slice::from_raw_parts_mut;

            from_raw_parts_mut(
                (gptbr.baddr() << 12) as *mut _,
                1 << (config
                    .pps
                    .width()
                    .checked_sub(config.l0gptsz.width())
                    .ok_or(Error::InvalidConfiguration)?),
            )
        };

        Ok(Self {
            level0: Level0Table(level0),
            config,
        })
    }

    /// Enables the Granule Protection Checks using this Granule Protection Table.
    ///
    /// # Safety
    ///
    /// Before calling this function, the caller must ensure that the table grants access to the
    /// Root World for the whole RF-A address space.
    pub unsafe fn enable(&self, config: Option<GpccrConfig>) -> Result<(), Error> {
        let mut gpcc = match config {
            Some(c) => c.to_reg(),
            None => read_gpccr_el3(),
        };
        assert!(!gpcc.contains(GpccrEl3::GPC));

        gpcc.set_pps(self.config.pps as u8);
        gpcc.set_l0gptsz(self.config.l0gptsz as u8);
        gpcc.set_pgs(self.config.pgs as u8);

        let base = self.level0.0.as_ptr() as u64;
        if base & mask!(12) != 0 {
            return Err(Error::MisalignedL0Buffer);
        }

        let mut gptbr = GptbrEl3::empty();
        gptbr.set_baddr(base >> 12);

        // Writes the register, except for the Granule Protection Check enabled bit.
        // SAFETY: since the GPC bit is off, this operation has no effect.
        unsafe {
            write_gptbr_el3(gptbr);
            write_gpccr_el3(gpcc);
        }

        isb();
        tlbi_paallos();
        dsbsy();
        isb();

        gpcc |= GpccrEl3::GPC;

        // Safety: Root World access is ensured by the caller. The pointer in `GPTBR_EL3` was
        // previously configured with the address of a valid Level 0 Table.
        unsafe {
            write_gpccr_el3(gpcc);
        }

        // Invalidate TLB entries.
        isb();
        tlbi_paallos();
        dsbsy();
        isb();

        Ok(())
    }
}

/// Configuration for the `GPCCR_EL3` registers.
pub struct GpccrConfig {
    /// Above PPS All Access.
    ///
    /// If set to true, accesses to addresses outside of the Physical Protected Space will not cause
    /// a fault.
    pub appsaa: bool,
    /// Trace Buffer Granule Protection Check Disabled.
    ///
    /// Controls whether the Trace Buffer Unit accepts or rejects trace when Granule Protection
    /// Checks are disabled.
    pub tbgpcd: bool,
    /// Granule Protection Check Priority.
    ///
    /// This control governs behavior of granule protection checks on fetches of stage 2 Table
    /// descriptors.
    ///
    /// - `false`: GPC faults are all reported with a priority that is consistent with the GPC being
    ///   performed on any access to physical address space.
    /// - `true`: A GPC fault for the fetch of a Table descriptor for a stage 2 translation table
    ///   walk might not be generated or reported. All other GPC faults are reported with a priority
    ///   consistent with the GPC being performed on all accesses to physical address spaces.
    pub gpcp: bool,
    /// GPT fetch Shareability attribute.
    pub sh: Shareability,
    /// GPT fetch Outer cacheability attribute.
    pub orgn: Cacheability,
    /// GPT fetch Inner cacheability attribute.
    pub irgn: Cacheability,
    /// Secure PA space Disable.
    pub spad: bool,
    /// Non-seucre PA space disable.
    pub nspad: bool,
    /// Realm-secure PA space disable.
    pub rlpad: bool,
    // TODO: handle NSO bit properly (through a dedicated feature?) to expose `GPIAccessType::NSO`
    // accordingly.
}

impl GpccrConfig {
    fn to_reg(&self) -> GpccrEl3 {
        let mut reg = GpccrEl3::empty();

        if self.appsaa {
            reg |= GpccrEl3::APPSAA
        }

        if self.tbgpcd {
            reg |= GpccrEl3::TBGPCD
        }

        if self.gpcp {
            reg |= GpccrEl3::GPCP
        }

        if self.spad {
            reg |= GpccrEl3::SPAD
        }

        if self.nspad {
            reg |= GpccrEl3::NSPAD
        }

        if self.rlpad {
            reg |= GpccrEl3::RLPAD
        }

        reg.set_sh(self.sh);
        reg.set_orgn(self.orgn);
        reg.set_irgn(self.irgn);

        reg
    }
}
