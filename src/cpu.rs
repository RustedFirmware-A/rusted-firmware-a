// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

macro_rules! add_cpu_mod {
    ($module:ident) => {
        #[cfg(all(target_arch = "aarch64", not(test)))]
        pub mod $module;
    };
}

add_cpu_mod!(aem_generic);
add_cpu_mod!(qemu_max);

use crate::platform::CPU_OPS;
use arm_sysregs::read_midr_el1;

/// The `Cpu` trait captures low level CPU specific operations.
/// # Safety
///
/// Except in unit-test-only builds for the host, the `reset_handler` function must be implemented
/// as a naked function. It should only clobber X0-X18, and X30 registers. It must not use the
/// stack, because it might be called at a point when the stack has not been configured.
pub unsafe trait Cpu {
    /// Main ID register value, only the 'Implementer' and 'PartNum' fields are used for identifying
    /// the `Cpu` implementation.
    const MIDR: u64;

    /// This function is called on CPU cold boot.
    extern "C" fn reset_handler();

    /// Prepares for a power down that only affects power level 0.
    fn power_down_level0();

    /// Prepares for a power down that affects power level 0 and 1.
    fn power_down_level1();
}

/// Structure for storing MIDR value and `CpuOps` function pointers.
#[repr(C)]
#[derive(Debug)]
pub struct CpuOps {
    midr: u64,
    reset_handler: extern "C" fn(),
    power_down_level0: fn(),
    power_down_level1: fn(),
}

impl CpuOps {
    /// Only use Implementer and PartNum fields.
    const MIDR_MASK: u64 = 0xff00_fff0;

    /// Check if the instance has an MIDR with matching Implementer and PartNum fields.
    fn has_matching_midr(&self, midr: u64) -> bool {
        self.midr == (midr & Self::MIDR_MASK)
    }
}

impl CpuOps {
    /// Create [CpuOps] from [Cpu] implementation.
    pub const fn from_cpu<T: Cpu>() -> Self {
        Self {
            midr: T::MIDR & Self::MIDR_MASK,
            reset_handler: T::reset_handler,
            power_down_level0: T::power_down_level0,
            power_down_level1: T::power_down_level1,
        }
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

/// Declares the CPU_OPS array.
macro_rules! define_cpu_ops {
    ($($cpus:ty),+) => {
        pub static CPU_OPS : [$crate::cpu::CpuOps; $crate::cpu::cpu_ops_count!($($cpus),+)] = [
            $($crate::cpu::CpuOps::from_cpu::<$cpus>()),*,
        ];
    }
}
pub(crate) use define_cpu_ops;

fn find_cpu_ops() -> &'static CpuOps {
    let midr = read_midr_el1();
    let ops = CPU_OPS.iter().find(|i| i.has_matching_midr(midr));

    match ops {
        Some(ops) => ops,
        None => panic!("Unknown MIDR {midr:x}"),
    }
}

#[cfg(test)]
pub extern "C" fn cpu_reset_handler() {
    let ops = find_cpu_ops();

    (ops.reset_handler)()
}

#[cfg(not(test))]
#[unsafe(naked)]
#[unsafe(no_mangle)]
pub extern "C" fn cpu_reset_handler() {
    crate::naked_asm!(
        "/* Read and mask MIDR_EL1 */
        mrs	x2, midr_el1
        mov	w3, ({midr_mask} & 0xffff)
        movk	w3, (({midr_mask} >> 16) & 0xffff), LSL 16
        and	w2, w2, w3

        /* Get address of the beginning of CPU_OPS */
        ldr	x3, ={cpu_ops}

        /* Get address of the end of CPU_OPS */
        ldr	x4, =({cpu_ops} + ({cpu_ops_size} * {cpu_ops_count}))

    1:
        /* Load the midr from the CPU_OPS */
        ldr	w1, [x3, #{midr_offset}]

        /* Check if MIDR matches the MIDR of this core */
        cmp	w1, w2
        b.eq	2f

        /* Check end of list */
        cmp	x3, x4
        b.eq	3f

        /* Step to next CPU_OPS entry */
        add	x3, x3, #{cpu_ops_size}
        b	1b

    2:
        /* Read and jump to reset handler function */
        ldr	x1, [x3, #{reset_handler_offset}]
        br	x1

    3:
        /* The MIDR values was not found */
        b	el3_panic
        ",
        midr_mask = const CpuOps::MIDR_MASK,
        cpu_ops = sym CPU_OPS,
        cpu_ops_size = const core::mem::size_of::<CpuOps>(),
        cpu_ops_count = const CPU_OPS.len(),
        midr_offset = const core::mem::offset_of!(CpuOps, midr),
        reset_handler_offset = const core::mem::offset_of!(CpuOps, reset_handler),
    );
}

pub fn cpu_power_down(level: usize) {
    let ops = find_cpu_ops();

    if level == 0 {
        (ops.power_down_level0)()
    } else {
        (ops.power_down_level1)()
    };
}

#[cfg(test)]
mod test {
    use super::*;
    use arm_sysregs::fake::SYSREGS;

    #[test]
    fn test_reset_handler() {
        SYSREGS.lock().unwrap().midr_el1 = 0;
        cpu_reset_handler();
        cpu_power_down(0);
        cpu_power_down(1);
    }
}
