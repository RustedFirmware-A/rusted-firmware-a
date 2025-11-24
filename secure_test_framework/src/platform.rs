// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

#[cfg(platform = "fvp")]
mod fvp;
#[cfg(platform = "qemu")]
mod qemu;

use arm_gic::gicv3::GicV3;
use arm_sysregs::{MpidrEl1, read_mpidr_el1};
use core::fmt::Write;
#[cfg(platform = "fvp")]
#[allow(unused)]
pub use fvp::{BL32_IDMAP, BL33_IDMAP, RMM_IDMAP};
use percore::Cores;
#[cfg(platform = "qemu")]
#[allow(unused)]
pub use qemu::{BL32_IDMAP, BL33_IDMAP};

#[cfg(platform = "fvp")]
pub type PlatformImpl = fvp::Fvp;
#[cfg(platform = "qemu")]
pub type PlatformImpl = qemu::Qemu;

/// The hooks implemented by each platform.
///
/// # Safety
///
/// `core_position` must be a naked function which doesn't access any memory, and must never return
/// the same index for two different valid MPIDR values. It must only clobber x0-x3.
pub unsafe trait Platform {
    /// The number of CPU cores.
    const CORE_COUNT: usize;

    /// Returns something to which logs should be sent.
    ///
    /// This should only be called once, and may panic on subsequent calls.
    fn make_log_sink() -> &'static mut (dyn Write + Send);

    /// Returns the GIC instance of the platform.
    ///
    /// # Safety
    ///
    /// This must only be called once, to avoid creating aliases of the GIC driver.
    unsafe fn create_gic() -> GicV3<'static>;

    /// Given a valid MPIDR value, returns the corresponding linear core index.
    ///
    /// The implementation must never return the same index for two different valid MPIDR values,
    /// and must never return a value greater than or equal to the corresponding
    /// `Platform::CORE_COUNT`. It must return 0 for the primary core, i.e. the core which powers on
    /// first and handles initialisation.
    ///
    /// For an invalid MPIDR value no guarantees are made about the return value.
    extern "C" fn core_position(mpidr: MpidrEl1) -> usize;

    /// Given a linear core index, returns the corresponding PSCI MPIDR value.
    ///
    /// This is not quite the inverse function of `core_position`, as it doesn't include the MT and
    /// U bits which `core_position` may expect.
    fn psci_mpidr_for_core(core_index: usize) -> u64;

    /// Returns the topology description for OSI tests.
    ///
    /// The returned slice should contain the core count for each cluster.
    fn osi_test_topology() -> &'static [usize] {
        unimplemented!("OSI topology not implemented for this platform")
    }

    /// Constructs a platform-specific power state value for OSI tests.
    ///
    /// This combines the `state_id` (representing the type of state, e.g., power down or standby)
    /// and the `last_level` (the highest power level that will lose power) into a composite
    /// value expected by the `CPU_SUSPEND` SMC.
    fn make_osi_power_state(_state_id: u32, _last_level: u32) -> u32 {
        unimplemented!("OSI power state construction not implemented")
    }

    /// Returns a list of invalid power states to test against.
    ///
    /// These states are used to verify that the implementation correctly rejects invalid
    /// parameters in OSI mode.
    fn osi_invalid_power_states() -> &'static [u32] {
        &[]
    }

    /// Returns the State ID for a core power down state (Affinity Level 0).
    fn osi_state_id_core_power_down() -> u32 {
        unimplemented!("OSI state ID not implemented")
    }

    /// Returns the State ID for a cluster power down state (Affinity Level 1).
    fn osi_state_id_cluster_power_down() -> u32 {
        unimplemented!("OSI state ID not implemented")
    }

    /// Returns the State ID for a system power down state (Affinity Level 2+).
    fn osi_state_id_system_power_down() -> u32 {
        unimplemented!("OSI state ID not implemented")
    }

    /// Returns the State ID for a core standby/retention state.
    fn osi_state_id_core_standby() -> u32 {
        unimplemented!("OSI state ID not implemented")
    }

    /// Returns the duration in timer ticks for which the test should suspend the CPU.
    ///
    /// This value is used to program the wake-up timer.
    fn osi_suspend_duration_ticks() -> u32 {
        200_000
    }

    /// Returns the delay in microseconds to wait before a secondary core enters suspend.
    ///
    /// This delay is used in tests to ensure that the primary core has time to update
    /// coordination status or to stagger the suspend requests of multiple cores.
    fn osi_suspend_entry_delay_us() -> u64 {
        5_000
    }

    /// Returns true if the given core should be woken up for OSI tests.
    fn osi_should_wake_core(_core_index: usize) -> bool {
        true
    }
}

// SAFETY: `Platform::core_position` is guaranteed to return a unique value for any valid MPIDR
// value.
unsafe impl Cores for PlatformImpl {
    fn core_index() -> usize {
        Self::core_position(read_mpidr_el1())
    }
}
