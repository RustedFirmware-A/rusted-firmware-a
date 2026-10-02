// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Framework for working around CPU and other errata.

use core::{fmt::Debug, num::NonZero};

#[cfg(all(target_arch = "aarch64", not(any(test, feature = "fakes"))))]
pub mod dsu;

/// A unique identifier for an erratum.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ErratumId(NonZero<u32>);

impl ErratumId {
    /// Initializes an Erratum ID
    pub const fn new(value: u32) -> Self {
        Self(NonZero::new(value).unwrap())
    }

    /// Returns the contained value as a primitive type.
    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

impl TryFrom<u32> for ErratumId {
    type Error = <NonZero<u32> as TryFrom<u32>>::Error;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        value.try_into().map(Self)
    }
}

/// The CVE number associated with an erratum.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct CveId(NonZero<u32>);

impl CveId {
    const CVE_NUM_BITS: usize = 20; // works for numbers up to 1048575
    const CVE_YEAR_BITS: usize = 12; // works for years up to 4095
    const CVE_YEAR_SHIFT: usize = Self::CVE_NUM_BITS;
    const CVE_NUM_MAX: u32 = 1 << Self::CVE_NUM_BITS;
    const CVE_YEAR_MAX: u32 = 1 << Self::CVE_YEAR_BITS;

    /// Initializes a CVE ID from year and number
    pub const fn new(year: u32, num: u32) -> Self {
        assert!(year != 0 && year < Self::CVE_YEAR_MAX);
        assert!(num != 0 && num < Self::CVE_NUM_MAX);
        Self(NonZero::new(year << Self::CVE_YEAR_SHIFT | num).unwrap())
    }
}

impl Debug for CveId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Some(CVE-{}-{:04})",
            self.0.get() >> Self::CVE_YEAR_SHIFT,
            self.0.get() & ((1 << Self::CVE_YEAR_SHIFT) - 1)
        )
    }
}

/// Represents a CPU revision and variant.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub struct RevisionVariant {
    /// The CPU revision.
    pub revision: u8,
    /// The CPU variant.
    pub variant: u8,
}

impl RevisionVariant {
    /// Creates a new RevisionVariant.
    pub const fn new(revision: u8, variant: u8) -> Self {
        Self { revision, variant }
    }

    /// A sentinel value for errata that are not yet fixed.
    pub const NOT_FIXED: Self = Self::new(u8::MAX, u8::MAX);
}

/// Specifies when an erratum workaround should be applied.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum ErratumType {
    /// Apply the workaround at CPU reset, before the stack is set up.
    Reset,
    /// Apply the workaround at runtime by calling the function directly in the Platform
    /// implementation.
    Runtime,
}

/// Defines an interface for handling CPU errata, including identification,
/// vulnerability mapping (CVE), application type, and methods for checking
/// applicability and applying workarounds.
///
/// # Safety
///
/// Check and workaround function implementations should be naked functions that don't require a
/// stack and don't access memory. Check function may clobber x0-x4, workaround may clobber x0-x7.
pub unsafe trait Erratum {
    /// The unique ID of the erratum workaround.
    const ID: Option<ErratumId> = None;

    /// The CVE number of the erratum if it has one. Multiple errata may have the same CVE number.
    const CVE: Option<CveId> = None;

    /// The time at which the erratum workaround should be applied.
    const APPLY_ON: ErratumType;

    /// Returns true if the erratum should be applied.
    extern "C" fn check() -> bool;

    /// Applies the workaround for the erratum.
    extern "C" fn workaround();
}

/// A C-compatible struct of function pointers for an erratum.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct ErratumEntry {
    /// The unique ID of the erratum workaround.
    pub id: Option<ErratumId>,

    /// The CVE number of the erratum if it has one. Multiple errata may have the same CVE number.
    pub cve: Option<CveId>,

    /// The time at which the erratum workaround should be applied.
    pub apply_on: ErratumType,

    /// Returns true if the erratum should be applied.
    pub check: extern "C" fn() -> bool,

    /// Applies the workaround for the erratum.
    pub workaround: extern "C" fn(),
}

impl ErratumEntry {
    /// Creates an ErratumEntry struct from an implementation of the Erratum trait.
    pub const fn from_erratum<T: Erratum>() -> Self {
        const {
            assert!(
                T::ID.is_some() || T::CVE.is_some(),
                "Every erratum entry must have at least one of ID or CVE set."
            )
        }
        Self {
            id: T::ID,
            cve: T::CVE,
            apply_on: T::APPLY_ON,
            check: T::check,
            workaround: T::workaround,
        }
    }
}

/// Returns an erratum entry by ErratumId, if present. Note that the returned erratum may not apply
/// on the current CPU.
pub const fn erratum_by_id(errata_list: &[ErratumEntry], id: ErratumId) -> Option<&ErratumEntry> {
    // Must do iteration manually, since iterators aren't const compatible yet.
    // This is equivalent to `errata_list.iter().find(|erratum| erratum.id == Some(id))`
    let mut i = 0;
    while i < errata_list.len() {
        if let Some(id2) = errata_list[i].id
            && id2.get() == id.get()
        {
            return Some(&errata_list[i]);
        }
        i += 1
    }
    None
}

/// Returns true if the given errata list contains an erratum ID, and it applies on the current CPU.
pub fn erratum_applies(errata_list: &[ErratumEntry], id: ErratumId) -> bool {
    matches!(erratum_by_id(errata_list, id), Some(erratum) if (erratum.check)())
}

/// Returns true if the errata list contains an erratum with the given CVE number that applies on
/// the current CPU. Note that a false result could mean that the CVE is known on the current CPU
/// but fixed in hardware.
pub fn cve_applies(errata_list: &[ErratumEntry], cve: CveId) -> bool {
    errata_list
        .iter()
        .any(|erratum| erratum.cve == Some(cve) && (erratum.check)())
}

/// Methods to access the errata for the platform.
///
/// Implemented for the platform by the `define_errata_list!` macro, platforms shouldn't implement
/// it manually.
///
/// # Safety
///
/// Other than in unit tests, the `apply_reset_errata` method must be implemented as a naked
/// function which doesn't use the stack or clobber any registers other than x0-x11.
pub unsafe trait PlatformErrata: Send + Sync {
    /// The list of all errata workarounds which may need to be applied on the platform.
    const ERRATA_LIST: &'static [ErratumEntry];

    /// Iterates over the ERRATA_LIST, calling the check function on each reset erratum and then
    /// calling the workaround function if the check function returned true.
    ///
    /// Clobbers registers x0-x11.
    #[cfg(not(test))]
    extern "C" fn apply_reset_errata();
}

/// Calculates the count of specified Erratum types.
#[macro_export]
macro_rules! errata_count {
    () => { 0 };
    ($erratum:ty) => { 1 };
    ($erratum:ty, $($errata:ty),+) => {
        $crate::errata_framework::errata_count!($erratum) + $crate::errata_framework::errata_count!($($errata),+)
    };
}
pub use errata_count;

/// Declares the ERRATA_LIST array.
#[macro_export]
macro_rules! define_errata_list {
    ($platform:ty, [$($erratum:ty),*]) => {
        static ERRATA_LIST : [$crate::errata_framework::ErratumEntry; $crate::errata_framework::errata_count!($($erratum),*)] = [
            $($crate::errata_framework::ErratumEntry::from_erratum::<$erratum>()),*
        ];

        const _: () = {
            let mut i = 0;
            while i < ERRATA_LIST.len() {
                if ERRATA_LIST[i].id.is_none() {
                    i += 1;
                    continue;
                }
                let mut j = i + 1;
                while j < ERRATA_LIST.len() {
                    if ERRATA_LIST[j].id.is_none() {
                        j += 1;
                        continue;
                    }
                    assert!(
                        ERRATA_LIST[i].id.unwrap().get() != ERRATA_LIST[j].id.unwrap().get(),
                        "ERRATA_LIST contains multiple entries with the same ErratumId",
                    );
                    j += 1;
                }
                i += 1;
            }
        };

        // SAFETY: apply_reset_errata is a naked function and only clobbers x0-x11.
        unsafe impl $crate::errata_framework::PlatformErrata for $platform {
            const ERRATA_LIST: &'static [$crate::errata_framework::ErratumEntry] = &ERRATA_LIST;

            #[cfg(not(test))]
            #[unsafe(naked)]
            extern "C" fn apply_reset_errata() {
                use core::mem::offset_of;
                use $crate::errata_framework::{ErratumEntry, ErratumType};

                #[cfg(target_arch = "aarch64")]
                $crate::naked_asm!(
                    // Save LR
                    "mov x8, x30",
                    // Loop through the ERRATA slice.
                    // Address of the beginning of ERRATA_LIST.
                    "ldr x9, ={errata_list}",
                    "ldr x10, =({errata_list} + {erratum_entry_size} * {errata_list_count})",
                    "1:",
                    "cmp  x9, x10",
                    "b.eq 3f", // End of loop

                    // Load apply_on field
                    "ldr  w11, [x9, #{apply_on_offset}]",
                    "cmp  w11, {reset_type}",
                    "b.ne 2f", // Skip if not Reset type

                    // Call check()
                    "ldr  x11, [x9, #{check_offset}]",
                    "blr  x11",
                    "cbz  x0, 2f", // Skip if check() returns false

                    // Call workaround()
                    "ldr  x11, [x9, #{workaround_offset}]",
                    "blr  x11",

                    "2:",
                    "add  x9, x9, #{erratum_entry_size}", // Next entry
                    "b    1b",

                    "3:",
                    // Restore LR
                    "mov x30, x8",
                    "ret",

                    errata_list = sym ERRATA_LIST,
                    erratum_entry_size = const size_of::<ErratumEntry>(),
                    errata_list_count = const ERRATA_LIST.len(),
                    apply_on_offset = const offset_of!(ErratumEntry, apply_on),
                    check_offset = const offset_of!(ErratumEntry, check),
                    workaround_offset = const offset_of!(ErratumEntry, workaround),
                    reset_type = const ErratumType::Reset as u32,
                );
            }
        }
    }
}
pub use define_errata_list;

/// Implements the logic for an erratum `check` function in assembly.
///
/// This macro is suitable for an erratum applying to a single CPU
/// (possibly across a range of revisions / variants of this CPU).
/// For other more complex cases (e.g. erratum applying to multiple CPUs
/// or non-CPU IPs) a custom check() function should be implemented instead.
///
/// Clobbers registers x0-x4.
///
/// This macro generates an assembly implementation for the `check` function of an `Erratum`
/// implementation. It reads the MIDR_EL1 of the CPU, compares the part number and implementer
/// numbers against the given `$midr`, then compares the revision and variant numbers of the CPU
/// against `$apply_from` and `$fixed_in`. If the erratum is not yet fixed, then
/// `RevisionVariant::NOT_FIXED` can be passed for `$fixed_in`.
///
/// Note that the revision-variant check implemented here is for a half-open range, unlike
/// `check_erratum_range` in C TF-A which checks a closed range. In other words, the `$fixed_in`
/// value passed to this macro should be one higher than the `_rev_num_hi` passed to
/// `check_erratum_range`.
///
/// The generated assembly returns `true` (1) if the erratum should be applied,
/// and `false` (0) otherwise, following the AArch64 C calling convention.
///
/// # Example
///
/// ```compile_fail
/// struct MyErratum;
///
/// unsafe impl Erratum for MyErratum {
///     // ...
///     #[unsafe(naked)]
///     extern "C" fn check() -> bool {
///         implement_erratum_check!(MIDR, RevisionVariant::new(0, 1), RevisionVariant::NOT_FIXED)
///     }
///     // ...
/// }
/// ```
#[macro_export]
macro_rules! implement_erratum_check {
    ($midr:expr, $apply_from:expr, $fixed_in:expr $(,)?) => {
        {
            use arm_sysregs::el1::registers::MidrEl1;

            const MIDR__: MidrEl1 = $midr;
            const APPLY_FROM__: $crate::errata_framework::RevisionVariant = $apply_from;
            const FIXED_IN__: $crate::errata_framework::RevisionVariant = $fixed_in;
            /// The length of the variant field in bits.
            const VARIANT_LEN: u32 = MidrEl1::VARIANT_MASK.count_ones();
            /// The length of the revision field in bits.
            const REVISION_LEN: u32 = MidrEl1::REVISION_MASK.count_ones();

            $crate::naked_asm!(
                include_str!("../asm_macros_common.S"),
                // Read MIDR_EL1
                "mrs x1, midr_el1",

                // Check if implementer and part number agree with `midr`.
                "mov_imm x4, {midr_mask}",
                "and x2, x1, x4",
                "mov_imm x3, {midr_val}",
                "and x3, x3, x4",
                "cmp x2, x3",
                "b.ne 1f",

                // Load variant into bits [3:0] and revision into bits [7:4] of x2
                "ubfx x2, x1, #{midr_variant_shift}, #{midr_variant_len}",
                "bfi x2, x1, #{midr_variant_len}, #{midr_revision_len}",

                // Load APPLY_FROM with variant in bits [3:0] and revision in bits [7:4] into x3
                "mov x3, #{apply_from}",

                // Compare current revision and variant with APPLY_FROM
                "cmp x2, x3", // Compare current with apply_from
                "b.lt 1f",    // If less, condition not met, return false, else check against
                              // FIXED_IN

                // Load FIXED_IN with variant in bits [3:0] and revision in bits [7:4] into x4
                "mov x4, #{fixed_in}",

                // Compare current revision and variant with FIXED_IN
                "cmp x2, x4", // Compare current with fixed_in
                "b.ge 1f",    // If greater or equal, condition not met, return false, else return
                              // true

                // Return true
                "mov x0, #1",
                "ret",

                "1:", // Return false
                "mov x0, #0",
                "ret",

                include_str!("../asm_macros_common_purge.S"),
                DEBUG = const $crate::debug::DEBUG as i32, // Required by asm_macros_common.S
                apply_from = const (((APPLY_FROM__.revision as u32) << VARIANT_LEN) | APPLY_FROM__.variant as u32),
                fixed_in = const (((FIXED_IN__.revision as u32) << VARIANT_LEN) | FIXED_IN__.variant as u32),
                midr_revision_len = const REVISION_LEN as u32,
                midr_variant_shift = const MidrEl1::VARIANT_SHIFT as u32,
                midr_variant_len = const VARIANT_LEN,
                midr_val = const MIDR__.bits(),
                midr_mask = const MidrEl1::IMPLEMENTER_MASK << MidrEl1::IMPLEMENTER_SHIFT
                    | MidrEl1::PARTNUM_MASK << MidrEl1::PARTNUM_SHIFT,
            )
        }
    };
}
pub use implement_erratum_check;
