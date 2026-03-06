// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Framework for CPU specific operations.

macro_rules! add_cpu_mod {
    ($module:ident) => {
        #[cfg(all(target_arch = "aarch64", not(test)))]
        pub mod $module;
    };
}

add_cpu_mod!(aem_generic);
add_cpu_mod!(c1_pro);
add_cpu_mod!(c1_ultra);
add_cpu_mod!(qemu_max);

use arm_sysregs::MidrEl1;

/// The `Cpu` trait captures low level CPU specific operations.
///
/// # Safety
///
/// Except in unit-test-only builds for the host, the `reset_handler` function must be implemented
/// as a naked function. It should only clobber X0-X18, and X30 registers. It must not use the
/// stack, because it might be called at a point when the stack has not been configured.
///
/// Likewise, the `dump_registers` function must be a naked function, and only clobber x0-x6 and
/// x8-x15. It mustn't use the stack.
pub unsafe trait Cpu {
    /// Main ID register value, only the 'Implementer' and 'PartNum' fields are used for identifying
    /// the `Cpu` implementation.
    const MIDR: MidrEl1;

    /// This function is called on CPU cold boot.
    extern "C" fn reset_handler();

    /// Dumps CPU-specific registers as part of a crash dump.
    ///
    /// Returns the register name list in x6, and register values in x8-x15. Clobbers x0-x5.
    extern "C" fn dump_registers();

    /// Prepares for a power down that only affects power level 0.
    fn power_down_level0();

    /// Prepares for a power down that affects power level 0 and 1.
    fn power_down_level1();

    /// Unwinds architectural state set by the above power down functions in the event of a power
    /// down abandon. Default implementation assumes that the CPU does not support powerdown abandon
    /// and will panic since it should be impossible to wake from a power down wfi.
    fn handle_power_down_abandon() {
        panic!("Unexpected wake up from WFI!");
    }
}

/// Structure for storing MIDR value and `CpuOps` function pointers.
#[repr(C)]
#[derive(Debug)]
pub struct CpuOps {
    /// The MIDR value to identify the CPU.
    ///
    /// Note that only the bits included in [`Self::MIDR_MASK`] will be compared.
    pub midr: MidrEl1,
    reset_handler: extern "C" fn(),
    dump_registers: extern "C" fn(),
    power_down_level0: fn(),
    power_down_level1: fn(),
    handle_power_down_abandon: fn(),
}

impl CpuOps {
    /// Only use Implementer and PartNum fields.
    pub const MIDR_MASK: MidrEl1 = MidrEl1::from_bits_retain(0xff00_fff0);

    /// Check if the instance has an MIDR with matching Implementer and PartNum fields.
    pub fn has_matching_midr(&self, midr: MidrEl1) -> bool {
        self.midr == midr & Self::MIDR_MASK
    }
}

impl CpuOps {
    /// Create [CpuOps] from [Cpu] implementation.
    pub const fn from_cpu<T: Cpu>() -> Self {
        Self {
            midr: T::MIDR.intersection(Self::MIDR_MASK),
            reset_handler: T::reset_handler,
            dump_registers: T::dump_registers,
            power_down_level0: T::power_down_level0,
            power_down_level1: T::power_down_level1,
            handle_power_down_abandon: T::handle_power_down_abandon,
        }
    }
}

/// Methods to get `CpuOps` for the platform.
///
/// Implemented for the platform by the `define_cpu_ops!` macro, you shouldn't implement it
/// manually.
///
/// # Safety
///
/// The `get_cpu_ops` method must return either null or a valid `CpuOps` struct. Other than in unit
/// tests it must be implemented as a naked function which doesn't use the stack or clobber any
/// registers other than x0-x3.
pub unsafe trait PlatformCpuOps {
    /// The list of all CPUs included in this platform.
    const CPU_OPS: &'static [CpuOps];

    /// Looks up the `CpuOps` struct for the current CPU based on its MIDR.
    ///
    /// Returns a pointer to the `CpuOps` struct, or null if no matching `CpuOps` was found.
    ///
    /// Returns the CpuOps pointer in x0, and clobbers x1-x3.
    #[cfg(not(test))]
    extern "C" fn get_cpu_ops() -> *const CpuOps;
}

/// Returns the `CpuOps` matching the MIDR of the current CPU.
///
/// Panics if none is found.
fn find_cpu_ops<PlatformImpl: PlatformCpuOps>() -> &'static CpuOps {
    let midr = arm_sysregs::read_midr_el1();
    let ops = PlatformImpl::CPU_OPS
        .iter()
        .find(|i| i.has_matching_midr(midr));

    match ops {
        Some(ops) => ops,
        None => panic!("Unknown MIDR {midr:x}"),
    }
}

/// Calculates the count of specified Cpu types.
macro_rules! cpu_ops_count {
    ($cpu:ty) => { 1 };
    ($cpu:ty, $($cpus:ty),+) => {
        $crate::cpu::cpu_ops_count!($cpu) + $crate::cpu::cpu_ops_count!($($cpus),+)
    };
}
pub(crate) use cpu_ops_count;

/// Declares the CPU_OPS array for the platform, and implements `PlatformCpuOps` to access it.
macro_rules! define_cpu_ops {
    ($platform:ty, [$($cpus:ty),+]) => {
        static CPU_OPS : [$crate::cpu::CpuOps; $crate::cpu::cpu_ops_count!($($cpus),+)] = [
            $($crate::cpu::CpuOps::from_cpu::<$cpus>()),*,
        ];

        // SAFETY: get_cpu_ops returns a valid CpuOps and only clobbers the documented registers.
        unsafe impl $crate::cpu::PlatformCpuOps for $platform {
            const CPU_OPS: &'static [$crate::cpu::CpuOps] = &CPU_OPS;

            #[cfg(not(test))]
            #[unsafe(naked)]
            extern "C" fn get_cpu_ops() -> *const $crate::cpu::CpuOps {
                $crate::naked_asm!(
                    "/* Read and mask MIDR_EL1 */
                    mrs	x2, midr_el1
                    mov	w0, ({midr_mask} & 0xffff)
                    movk	w0, (({midr_mask} >> 16) & 0xffff), LSL 16
                    and	w2, w2, w0

                    /* Get address of the beginning of CPU_OPS */
                    ldr	x0, ={cpu_ops}

                    /* Get address of the end of CPU_OPS */
                    ldr	x3, =({cpu_ops} + ({cpu_ops_size} * {cpu_ops_count}))

                1:
                    /* Check end of list */
                    cmp	x0, x3
                    b.eq	2f

                    /* Load the midr from the CPU_OPS */
                    ldr	w1, [x0, #{midr_offset}]

                    /* Check if MIDR matches the MIDR of this core */
                    cmp	w1, w2
                    b.eq	3f

                    /* Step to next CPU_OPS entry */
                    add	x0, x0, #{cpu_ops_size}
                    b	1b

                2:
                    /* The MIDR value was not found. */
                    mov x0, xzr
                3:
                    ret",
                    midr_mask = const $crate::cpu::CpuOps::MIDR_MASK.bits(),
                    cpu_ops = sym CPU_OPS,
                    cpu_ops_size = const core::mem::size_of::<$crate::cpu::CpuOps>(),
                    cpu_ops_count = const CPU_OPS.len(),
                    midr_offset = const core::mem::offset_of!($crate::cpu::CpuOps, midr),
                );
            }
        }
    }
}
pub(crate) use define_cpu_ops;

/// Finds the CPU operations for the current CPU and calls the reset handler for it.
#[cfg(test)]
pub extern "C" fn cpu_reset_handler<PlatformImpl: PlatformCpuOps>() {
    let ops = find_cpu_ops::<PlatformImpl>();

    (ops.reset_handler)()
}

/// Finds the CPU operations for the current CPU and calls the reset handler for it.
#[cfg(not(test))]
#[unsafe(naked)]
pub extern "C" fn cpu_reset_handler<PlatformImpl: PlatformCpuOps>() {
    crate::naked_asm!(
        "mov	x4, x30
        bl	{get_cpu_ops}
        mov	x30, x4
        cbz	x0, 1f

        /* Read and jump to reset handler function */
        ldr	x1, [x0, #{reset_handler_offset}]
        br	x1

    1:
        /* The MIDR values was not found */
        b	el3_panic
        ",
        get_cpu_ops = sym PlatformImpl::get_cpu_ops,
        reset_handler_offset = const core::mem::offset_of!(CpuOps, reset_handler),
    );
}

/// Fetches up to 8 CPU-specific registers of the current CPU for a crash dump.
///
/// Returns the register name list in x6, and register values in x8-x15.
/// Clobbers x0-x5.
///
/// # Safety
///
/// Should only be called from assembly as it doesn't follow the standard calling convention.
#[cfg(not(test))]
#[unsafe(naked)]
pub unsafe extern "C" fn cpu_dump_registers<PlatformImpl: PlatformCpuOps>() {
    crate::naked_asm!(
        "mov	x4, x30
        bl	{get_cpu_ops}
        mov	x30, x4
        cbz	x0, 1f

        /* Read and jump to dump_registers function */
        ldr	x1, [x0, #{dump_registers_offset}]
        br	x1

    1:
        /*
         * The MIDR value was not found. We are already in the middle of a crash dump, so just
         * ignore rather than panicking recursively.
         */
        ret",
        get_cpu_ops = sym PlatformImpl::get_cpu_ops,
        dump_registers_offset = const core::mem::offset_of!(CpuOps, dump_registers),
    );
}

/// Finds the CPU operations for the current CPU and calls the power down hook for the given level.
pub fn cpu_power_down<PlatformImpl: PlatformCpuOps>(level: usize) {
    let ops = find_cpu_ops::<PlatformImpl>();

    if level == 0 {
        (ops.power_down_level0)()
    } else {
        (ops.power_down_level1)()
    };
}

/// Finds the CPU operations for the current CPU and calls the power down abandon hook for it.
pub fn cpu_handle_power_down_abandon<PlatformImpl: PlatformCpuOps>() {
    let ops = find_cpu_ops::<PlatformImpl>();
    (ops.handle_power_down_abandon)()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::platform::test::TestPlatform;
    use arm_sysregs::MidrEl1;
    use arm_sysregs::fake::SYSREGS;

    #[test]
    fn test_reset_handler() {
        SYSREGS.lock().unwrap().midr_el1 = MidrEl1::empty();
        cpu_reset_handler::<TestPlatform>();
        cpu_power_down::<TestPlatform>(0);
        cpu_power_down::<TestPlatform>(1);
    }
}
