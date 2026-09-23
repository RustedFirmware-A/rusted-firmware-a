# Architecture Extensions

## Required Architecture Extensions

RF-A assumes that it is run on Arm CPUs based on Armv9.0 or later extensions. This implies the
presence of extensions which are mandatory from Armv8.5; however, RF-A does not strictly require all
of these to run. The following table outlines architecture extensions *necessary* to run RF-A:

| Chapter | Architecture Extension Name | Optional From | Condition | Short description | Comments |
| ------- | --------------------------- | ------------- | --------- | ----------------- | -------- |
| **The Armv8.0 architecture extension** |  |  |  |  |  |
| | `FEAT_AA64` | Armv8.0 | Unconditional | PE uses AArch64 after last reboot | The project targets AArch64. |
| | `FEAT_EL3` | Armv8.0 | Unconditional | Support for EL3 |  |
| | `FEAT_PMUv3` | Armv8.0 | Unconditional | Performance Monitors Extension version 3 | Cycle and event counters are disabled by default. RF-A prohibits cycle and event counting in EL3 and in Secure World. |
| | `FEAT_Secure` | Armv8.0 | Unconditional | Support for Secure state |  |
| | `FEAT_TGran4K` | Armv8.0 | Unconditional | Support for 4KB memory translation granule size at stage 1 | Page tables use 4KB granules. |
| **The Armv8.4 architecture extension** |  |  |  |  |  |
| | `FEAT_DIT` | Armv8.3; mandatory from Armv8.4 | Unconditional | Data Independent Timing Instructions | EL3 uses Data-Independent Timing Instructions. |
| | `FEAT_SEL2` | Armv8.3 | When the `sel2` feature is enabled | Secure EL2 |  |
| **The Armv8.5 architecture extension** |  |  |  |  |  |
| | `FEAT_SB` | Armv8.0; mandatory from Armv8.5 | Unconditional | Speculation Barrier | Used after `ERET`. |
| **The Armv9.2 architecture extension** |  |  |  |  |  |
| | `FEAT_RME` | Armv9.1 | When the `rme` feature is enabled | Realm Management Extension |  |

## Implemented Architecture Extensions

The table below lists architecture extensions supported by RF-A.

The `Architecture Extension Name` column follows the naming convention in the Architecture
Extensions chapter of the [Architecture Reference Manual for A-profile architecture, DDI 0487][1].

The tables are grouped and ordered by the Architecture Extension chapters (A2.2, A2.3) in
[DDI 0487][1]. Within each chapter group, extensions are ordered as they appear in that chapter.

For each listed extension, the `Optional From` column records the first architecture version in
which it is available, and notes a mandatory version where applicable.

The terminology of the `Supported by RF-A` column:
 - `Implied`: a superset of the feature is supported,
 - `Yes`: fully supported.

The `Gating` column describes how the feature is enabled:
 - `Build`: no runtime feature checking, can be enabled/disabled via build parameters.
 - `Build + Runtime`: can be enabled/disabled via build parameters, with runtime feature checking.
 - `Platform`: no runtime feature checking, can be enabled/disabled based on platform policy.
 - `Platform + Runtime`: can be enabled/disabled based on platform policy, with runtime feature
   checking.
 - `Runtime`: present in binary, but availability is checked at runtime.
 - `Unconditional`: no runtime feature checking, present in binary regardless of build flags or
   platform settings.

In most cases, RF-A's responsibility is limited to managing architectural extensions on behalf of
lower Exception Levels (EL2/EL1/EL0), specifically discovering the feature capability and enabling
the extension, so lower ELs can use it without trapping to EL3.
The level of support sometimes depends on the security state and exception level.
`Lower EL enablement` follows the following convention:
 - `S` - extension is enabled for Secure World
 - `NS` - extension is enabled for Non-Secure World
 - `R` - extension is enabled for Realm World.

| Chapter | Architecture Extension Name | Optional From | Supported by RF-A | Gating | Lower EL enablement | Used by EL3 | Tested in STF | Short description | Comments |
| ------- | --------------------------- | ------------- | ----------------- | ------ | ------------------- | ----------- | ------------- | ----------------- | -------- |
| **The Armv8.0 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_AdvSIMD` | Armv8.0 | Yes | Platform + Runtime | S/NS/R | No | Yes | Support for `SISD` and `SIMD` operations | Enables lower EL SIMD for all worlds; saves NS/S/Realm state when not using S-EL2. `FPCR`, `FPSR`, and Q registers are context-switched. |
| | `FEAT_CSV2` | Armv8.0; mandatory from Armv8.5 | Implied |  | N/A | No | No | Cache Speculation Variant 2 | No code. See `FEAT_CSV2_2`. |
| | `FEAT_CSV2_2` | Armv8.0 | Yes | Platform + Runtime | S/NS/R | No | Yes | Cache Speculation Variant 2 version 2 | Enables lower EL access for all worlds. EL1 or EL2 register context switching of `SCXTNUM_ELx` by `sel2`. |
| | `FEAT_TRC_SR` | Armv8.0 | Yes | Platform + Runtime | NS | No | No | Trace System registers | Code helper name is `SYS_REG_TRACE`. |
| **The Armv8.1 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_LSE` | Armv8.0; mandatory from Armv8.1 | Yes | Build | N/A | Yes | No | Large System Extensions | By default, the compiler builds with the LSE flag. May be turned off by `LSE=0 make`. |
| | `FEAT_PAN` | Armv8.0; mandatory from Armv8.1 | Yes | Unconditional | S/NS/R | No | No | Privileged access never | Context-switched along with `SPSR`. See `FEAT_PAN3`. |
| **The Armv8.2 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_IESB` | Armv8.1 | Yes | Unconditional | N/A | Yes | No | Implicit Error Synchronization event | Sets `SCTLR_EL3.IESB`. |
| | `FEAT_RAS` | Armv8.0; mandatory from Armv8.2 | Yes | Platform | N/A | No | No | Reliability, Availability and Serviceability (RAS) Extension | Saves NS/S/Realm RAS status. EL1 or EL2 register context switching by `sel2`. |
| | `FEAT_SPE` | Armv8.1 | Yes | Platform + Runtime | NS | No | No | Statistical Profiling Extension | Enables NS, traps S/R world. |
| | `FEAT_SVE` | Armv8.2 | Yes | Platform + Runtime | S/NS/R\* | No | Yes | Scalable Vector Extension | \* Disables traps for Non-Secure world, Secure world if `sel2`, Realm world if `rme`. Context-switches Non-Secure `Z0-Z31`, `P0-P15`, `FFR`, `FPCR`, and `FPSR` when `sel2` is disabled; `FFR` is not saved/restored in Streaming SVE mode unless `FEAT_SME_FA64` is present. See `FEAT_AdvSIMD`. |
| **The Armv8.3 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_CCIDX` | Armv8.2 | Yes | Runtime | N/A | Yes | No | Extended cache index | Reads extended `CCSIDR_EL1` format for cache maintenance. |
| | `FEAT_PAuth` | Armv8.2; mandatory from Armv8.3 | Yes | Unconditional\* | N/A | Yes if the `pauth` feature is enabled | No | Pointer authentication | \* EL3 keys are enabled by the `pauth` feature flag. PAuth registers are context-switched unconditionally. |
| **The Armv8.4 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_AMUv1` | Armv8.3 | Yes | Platform + Runtime | NS | No | Yes | Activity Monitor Extension | Saves AMU on powerdown suspend. G0 counters 0-1 are always on; G0 counters 2-3 are context-switched and disabled in EL3. G1 counters cannot be enabled without `FEAT_AMUv1p1`. |
| | `FEAT_DIT` | Armv8.3; mandatory from Armv8.4 | Yes | Unconditional | S/NS/R | Yes | Yes | Data Independent Timing Instructions | Context-switched along with `SPSR`. |
| | `FEAT_MPAM` | Armv8.2 | Yes | Platform + Runtime | NS/R | No | No | Memory System Resource Partitioning and Monitoring | Enables lower EL NS/R; saves S/NS/R. Context-switches only when `sel2` is present. v0.1 unsupported. |
| | `FEAT_MPAMv1p0` | Armv8.2 | Yes | Platform + Runtime |  | No | No | Memory Partitioning and Monitoring Extension version 1.0 | See `FEAT_MPAM`. |
| | `FEAT_TRF` | Armv8.3 | Yes | Platform + Runtime | S/NS/R | No | No | Self-hosted Trace extensions |  |
| **The Armv8.5 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_BTI` | Armv8.4; mandatory from Armv8.5 | Yes | Build | N/A | Yes \* | No | Branch Target Identification | \* Inline assembly emits `bti` instructions, emitted Rust contains branch protection if `BTI_EL3=1` is set in the Makefile. A `bti c` instruction is emitted unconditionally by the `func` assembly macro. |
| | `FEAT_MTE` | Armv8.4 | Yes | Runtime | N/A | No | No | Memory Tagging Extension | Masks tag faults. |
| | `FEAT_MTE2` | Armv8.4 | Yes | Platform + Runtime | S/NS | No | No | Memory Tagging Extension 2 | Enables NS/S; saves S/NS/R. |
| | `FEAT_SSBS` | Armv8.0 | Yes | Runtime | N/A | No | No | Speculative Store Bypass Safe | Copies `DSSBS` to SPSR. |
| **The Armv8.6 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_AMUv1p1` | Armv8.5 | Yes | Platform + Runtime | NS | No | No | Activity Monitor Extension, with virtualization support | Implemented along with `FEAT_AMUv1`. Handles non-contiguous G1 counters via `AMCG1IDR_EL0`; can restrict G1 reads below EL3. Counter offsets are *not* context-switched. |
| | `FEAT_ECV` | Armv8.5; mandatory from Armv8.6 | Implied |  | N/A | No | No | Enhanced Counter Virtualization | See `FEAT_ECV_POFF`. |
| | `FEAT_ECV_POFF` | Armv8.5 | Yes | Unconditional | S/NS/R | No | No | Enhanced Counter Virtualization Physical Offset | Does not context-switch `CNTPOFF_EL2`. |
| | `FEAT_FGT` | Armv8.5 | Yes | Platform | NS/R | No | No | Fine-Grained Traps | Context-switches only when `sel2` or `rme` are present. |
| | `FEAT_MTPMU` | Armv8.5 | Yes | Platform + Runtime | S/NS/R | No | No | Multi-threaded PMU extensions | Enabled on all CPUs. See `FEAT_PMUv3`. |
| **The Armv8.7 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_HCX` | Armv8.6 | Yes | Platform + Runtime | S/NS/R in EL2 | No | Yes | Support for `HCRX_EL2` | Enables EL2 access in all worlds; saves S/NS/R. Zeroes `HCRX_EL2`; context-switches only when `sel2` is present. |
| | `FEAT_PAN3` | Armv8.1, mandatory from Armv8.7 | Yes | Unconditional | S/NS/R | No | Yes | Support for `SCTLR_ELx.EPAN`. | Context-switched along with `SCTLR`. |
| **The Armv8.8 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_NMI` | Armv8.7; mandatory from Armv8.8 | Yes | Runtime | S/NS/R | No | Yes | Non-maskable Interrupts | Context-switched along with `SPSR`. |
| | `FEAT_SCTLR2` | Armv8.0; mandatory from Armv8.9 | Yes | Platform + Runtime | S/NS/R | No | Yes | Extension to SCTLR_ELx | Enables lower EL access to all worlds; saves NS/S/Realm. Initializes `SCTLR2_EL3`. |
| | `FEAT_TCR2` | Armv8.0; mandatory from Armv8.9 | Yes | Platform + Runtime | S/NS/R | No | No | Support for `TCR2_ELx` | `TCR2_EL2` or `TCR2_EL1` context-switched depending on `sel2`. |
| **The Armv8.9 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_FGT2` | Armv8.8 | Yes | Platform + Runtime | S/NS/R | No | No | Fine-Grained Traps 2 | Context-switches only when `sel2` or `rme` are present. |
| | `FEAT_PFAR` | Armv8.8 | Yes | Platform + Runtime | S/NS/R | No | Yes | Physical Fault Address Registers | Enables lower EL support in all worlds; saves NS/S/Realm. |
| **The Armv9.0 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_TRBE` | Armv9.0 | Yes | Platform + Runtime | NS | No | No | Trace Buffer Extension | Enables trace buffer for NS world. Other accesses are trapped. |
| **The Armv9.2 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_RME` | Armv9.1 | Yes | Build + Runtime | N/A | Yes | Yes | Realm Management Extension | Enabled by the `rme` feature flag. |
| | `FEAT_SME` | Armv9.2 | Yes | Platform + Runtime | S/NS/R\* | No | No | Scalable Matrix Extension | \* Enables lower EL access for Non-Secure world, Secure world if `sel2`, Realm world if `rme`. Context-switches Non-Secure `SVCR` when `sel2` is disabled. Only when `Simd::sve(..., true)`. See `FEAT_SVE`. |
| | `FEAT_SME_FA64` | Armv9.2 | Yes | Platform + Runtime | S/NS/R\* | No | No | Full A64 instruction set support in Streaming SVE mode | \* Enables `SMCR_EL3.FA64` if present. Lower-level counterparts can be enabled through `SMCR_ELx`, which is only available with SME enabled. Secure world only when `sel2`, realm world only when `rme` is enabled. |
| **The Armv9.3 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_SME2` | Armv9.2 | Yes | Platform + Runtime | S/NS/R\* | No | No | Scalable Matrix Extensions version 2 | \* Enables ZT0 lower EL access if present, only when SME is enabled. |
| **The Armv9.4 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_EBEP` | Armv9.3 | Yes | Runtime | N/A | No | No | Exception-based Event Profiling |  |
| | `FEAT_GCS` | Armv9.3 | Yes | Platform + Runtime | S/NS/R | No | Yes | Guarded Control Stack | Enables lower EL access for all worlds. Context-switches `GCSCR` and `GCSPR`. |
| **The Armv9.5 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_FGWTE3` | Armv9.4 | Yes | Runtime | N/A | Yes | No | Fine-Grained Write Trap EL3 | Several EL3 registers locked after boot. |
| | `FEAT_FPMR` | Armv9.2 | Yes | Platform + Runtime | NS | No | No | Floating-Point Mode Register |  |
| | `FEAT_PAuth_LR` | Armv9.4 | Yes | Build + Runtime | N/A | No | No | Pointer authentication instructions that allow signing of LR using SP and PC as diversifiers |  |
| | `FEAT_RME_GPC2` | Armv9.4 | Yes | Build + Runtime | N/A | Yes | No | RME Granule Protection Check 2 Extension | Enabled by the `rme` feature flag. Used for access-type validation. |
| **The Armv9.6 architecture extension** |  |  |  |  |  |  |  |  |  |
| | `FEAT_RME_GDI` | Armv9.4 | Yes | Build + Runtime | N/A | Yes | No | RME Granular Data Isolation extension | Enabled by the `rme` feature flag. |

## Table Extension Guide

The table in [Required Architecture Extensions](#required-architecture-extensions) shall only be
extended with entries without which RF-A will crash at runtime. For example:
 - unconditionally writing to `UNDEFINED` registers (`FEAT_DIT`),
 - unconditionally calling instructions which are `UNDEFINED` (`FEAT_SB`),
 - assertions (`FEAT_RME` if the `rme` feature flag is selected).

[Implemented Architecture Extensions](#implemented-architecture-extensions) shall only be extended
with extensions which fulfill any of the following criteria:
 - RF-A contains explicit code,
 - STF contains explicit tests,
 - implied by another feature, and mandatory in the baseline version (see `FEAT_CSV2` and
   `FEAT_ECV`),
 - present in the output binary (see `FEAT_BTI` and `FEAT_LSE`).

[1]: https://support.arm.com/documentation/ddi0487/latest/

---

_Copyright The Rusted Firmware-A Contributors._
