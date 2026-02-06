// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    Error, GranuleProtection, GranuleProtectionConfig, Level0GptSize, Level0Table,
    PhysicalGranuleSize, ProtectedPhysicalAddressSize,
};
use arm_sysregs::{GpccrEl3, read_gpccr_el3, read_gptbr_el3};

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
}
