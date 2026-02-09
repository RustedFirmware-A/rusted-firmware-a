# Changelog

## v0.1.0 (2025-08-18)

This is the first tagged release of Rusted Firmware-A (RF-A). It introduces a Rust implementation
of EL3 runtime firmware (BL31) for Armv9-A and later systems, intended as a successor to TF-A's C
implementation.

### Firmware architecture and runtime

RF-A v0.1.0 provides a complete BL31 control loop for coordinating the Secure, Non-secure, and
(optionally) Realm worlds. The boot sequence performs platform bring-up (including early logging),
constructs an identity-mapped EL3 page table, enables the MMU, configures the GIC, initializes world
contexts, and then enters a world-switching runtime loop driven by exceptions returning to EL3
(SMCs, routed interrupts, and trapped system-register accesses).

Context management is per-core and saves/restores both general-purpose registers and lower-EL
architectural state. The saved lower-EL state is selected at build time: by default RF-A targets an
S-EL2 configuration (`sel2` Cargo feature), and can alternatively build without S-EL2. Exception
handling includes a defensive path for unknown trapped system-register accesses: instead of
panicking in EL3, RF-A can inject an Undefined exception back into the originating lower EL.

The memory-management implementation uses the `aarch64-paging` crate to build an EL3 translation
table and maps BL31 image ranges with appropriate permissions. Platform code contributes additional
device mappings. The GIC driver (based on the `arm-gic` crate) programs the distributor and
redistributors, and enables Group0/Group1 handling with a platform-defined interrupt configuration
table.

<details>
<summary>Commit list (128)</summary>

- log parameters to main function ([af6d831a68](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/af6d831a68))
- deny unsafe_op_in_unsafe_fn ([8b0270a148](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8b0270a148))
- install exception handlers ([2524da8386](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2524da8386))
- initialise page table and enable MMU ([536bafd2d4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/536bafd2d4))
- add functions to read and write system registers ([7eb087a427](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7eb087a427))
- initialise percpu_data pointer in tpidr_el3 ([084a25754a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/084a25754a))
- add constant to make clear that USER bit is RES1 ([e438dac93e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e438dac93e))
- remove #if conditions from copied assembly code ([f7ee821b82](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f7ee821b82))
- enable assertions in assembly code ([b3b451c523](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b3b451c523))
- add module for semihosting calls ([c022238e08](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c022238e08))
- set panic = "abort" for dev and release builds ([16c9c1c167](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/16c9c1c167))
- invalidate cache before enabling MMU ([5b0db3bb85](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5b0db3bb85))
- mair attributes name ([c22f0d5c8c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c22f0d5c8c))
- make normal cacheable memory inner shareable ([83e27b9e09](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/83e27b9e09))
- avoid global allocator ([4c64748441](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4c64748441))
- enter BL33 ([f1b4a9ccb8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f1b4a9ccb8))
- use fields rather than arrays for register state structs ([77a07d7483](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/77a07d7483))
- add safe abstraction for per-CPU data ([b82c3d2a8e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b82c3d2a8e))
- pass world rather than context to handle_smc ([e90bac4083](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e90bac4083))
- make targets to ease debugging ([dc34dc7851](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/dc34dc7851))
- use the right target name for `BL31_BIN` ([c8af6009a1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c8af6009a1))
- cleanups to sysregs fake ([63db4edb94](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/63db4edb94))
- start BL33 at EL2 rather than EL1 ([9416599ce2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9416599ce2))
- initialise secure world context too ([2c277e3b4c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2c277e3b4c))
- add EMPTY constant to CpuState ([e646bd4fbe](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e646bd4fbe))
- add EL2 system registers to CpuContext ([4cf7ecb20b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4cf7ecb20b))
- add macro to generate read and write sysreg together ([e683a9b75b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e683a9b75b))
- move function ID decoding logic to Rust ([8f38cba6e5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8f38cba6e5))
- avoid indirection for interrupt handling ([3d12dc6c0e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3d12dc6c0e))
- get current world index in Rust rather than assembly ([3870037c3e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3870037c3e))
- use aarch64-paging constants for MAIR ([1ec62000ae](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1ec62000ae))
- use `$crate` in macros ([9e9ecec4d1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9e9ecec4d1))
- make write_icc_sre_el3 unsafe ([0cbfaa5ea8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0cbfaa5ea8))
- add safety comments for some `unsafe` blocks ([4ac0ad79c4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4ac0ad79c4))
- start secure world first ([a71eb01ac8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a71eb01ac8))
- add a SAFETY comment for `impl Cores for TestPlatform` ([faa0e1b1c7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/faa0e1b1c7))
- remove unused constants ([664ba4270b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/664ba4270b))
- reimplement MMU configuration in Rust ([983d3a26a7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/983d3a26a7))
- rename World::current to from_scr and use constant in test ([0fe722cd60](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0fe722cd60))
- use underscores in long literals ([981ea547c5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/981ea547c5))
- module for general-use assembly helpers ([4d38ed53bc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4d38ed53bc))
- pass entry point to cpu ctx gp regs ([e827e41a95](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e827e41a95))
- make release builds a bit smaller ([204058ac1f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/204058ac1f))
- make S-EL2 optional ([f09f7deb0b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f09f7deb0b))
- allow system register type to be specified in macros ([244da64a2b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/244da64a2b))
- use bitflags types for some system registers ([af3141fe02](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/af3141fe02))
- add newtype for spsr_el3 ([b9a5e69390](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b9a5e69390))
- make `write_sctlr_el3` `unsafe` ([c99e53596f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c99e53596f))
- tidy up macro comments ([33dad95991](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/33dad95991))
- make `write_sctlr_el3` `unsafe` with comment ([7d335e1e14](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7d335e1e14))
- don't create a new object ([fd94d37229](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fd94d37229))
- set `missing_docs` lint to "deny" ([4e493ff03b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4e493ff03b))
- move `tlbi_alle3` to aarch64.rs ([1b50f425cf](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1b50f425cf))
- remove `anyhow` ([f4fd450e6c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f4fd450e6c))
- add function for reading ISR_EL1 ([8be54a65c4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8be54a65c4))
- put `undocumented_unsafe_blocks` in the right section ([68275c4c38](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/68275c4c38))
- log to stdout in test cfg ([7dbc4b1ad2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7dbc4b1ad2))
- pin cc to version that works ([6de1bca610](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6de1bca610))
- move tools to tools/ directory ([0b2ce40042](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0b2ce40042))
- require # Safety docs for `unsafe fn` ([a7df17add7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a7df17add7))
- make a comment exactly match the code ([5d0fcb0ae5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5d0fcb0ae5))
- implement power domain tree ([f117778673](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f117778673))
- implement CPU default suspend function ([3f70b7a02c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3f70b7a02c))
- enable `FEATURES` on the `make` command line ([57c7c7372a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/57c7c7372a))
- typo in a comment ([9220ffe5b5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9220ffe5b5))
- start on secure world test framework ([b23b088af6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b23b088af6))
- move plat_*_calc_core_pos to global_asm ([67e294f0ac](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/67e294f0ac))
- add framework to run tests by sending direct messages ([a6e1a8a251](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a6e1a8a251))
- reserve stack space for multiple cores ([c4d577d414](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c4d577d414))
- purge defined macros after use ([24f832fe88](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/24f832fe88))
- enable integer overflow checking ([3acf4485db](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3acf4485db))
- migrate debug.S into Rust ([0a537d3646](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0a537d3646))
- allow builders to set logging level ([b0796af434](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b0796af434))
- remove unused imports ([5597ff663a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5597ff663a))
- adopt Linux Foundation's guidance on copyrights ([8c9d7f7359](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8c9d7f7359))
- enable MMU on secondary cores ([0932859717](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0932859717))
- build context.S from Rust ([98559873b7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/98559873b7))
- add full bit assignment for ScrEl3 ([03d69a255f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/03d69a255f))
- update aarch64-paging, spin and bitflags ([49b40ec5cb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/49b40ec5cb))
- fix STF exception handling in EL2 ([fded3d8864](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fded3d8864))
- build crash_reporting.S from Rust ([be46563896](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/be46563896))
- build runtime_exceptions.S from Rust ([081e4ecd5d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/081e4ecd5d))
- remove unused assembly code functions ([e18973982e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e18973982e))
- sev doesn't affect the stack pointer ([0ff5b5396c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0ff5b5396c))
- inverted control flow and service instances ([de71e1f045](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/de71e1f045))
- move sysreg trap handling to Rust ([e97f1935a0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e97f1935a0))
- use Option::take rather than mem::swap ([b9df69dfae](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b9df69dfae))
- add initial interrupt handling ([31140814c5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/31140814c5))
- setup_mmu_cfg should be unsafe ([b6b90cc98e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b6b90cc98e))
- set interrupt routing model ([ed70d68f1a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ed70d68f1a))
- handle group 0 interrupts ([8ae3d698c7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8ae3d698c7))
- build cache_helpers.S from Rust ([e27f502fca](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e27f502fca))
- remove unused assembly macros ([8151659ee4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8151659ee4))
- add type alias for per-core mutable state ([c4153af438](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c4153af438))
- handle G1 interrupts in STF ([0a720b104b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0a720b104b))
- build misc_helpers.S from Rust ([4897f8bae0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4897f8bae0))
- enable EL2 interrupt handling in STF ([4d7c45bdf9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4d7c45bdf9))
- refactor main bl32 loop ([ba28a3253a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ba28a3253a))
- enable Secure EL1 access to timer ([d15be140dd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d15be140dd))
- less verbose logging ([46aa438fac](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/46aa438fac))
- move assembly files to rust directory ([346f64404e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/346f64404e))
- remove unused assembly macros ([0f6aa913f6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0f6aa913f6))
- move assembly headers under rust directory ([aeb4485de4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/aeb4485de4))
- fix misleading log message ([10e06e0c9c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/10e06e0c9c))
- implement ARM timer driver ([4232134383](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4232134383))
- update the project's rust version to 1.88 ([7b2137b37e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7b2137b37e))
- set write-through cache mode ([2e4583f0b9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2e4583f0b9))
- implement generic microsecond delay ([ef95e5b7eb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ef95e5b7eb))
- update aarch64-paging to 0.10.0 ([648f2cad1e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/648f2cad1e))
- remove unused assembly file ([aa8a2ee430](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/aa8a2ee430))
- log entire page table at debug level ([d09a4c5fac](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d09a4c5fac))
- update to Rust 2024 edition ([e93c39a346](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e93c39a346))
- exclude debug and trace logs from release builds ([8b82a0afa0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8b82a0afa0))
- remove IMAGE_BL31 flag from assembly code ([2388b65879](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2388b65879))
- introduce types for TestHelperProxy args and result ([21b46174bd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/21b46174bd))
- build bl31_entrypoint.S from Rust ([46e67a602e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/46e67a602e))
- remove unused functions ([1246aefd64](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1246aefd64))
- build cpu_helpers.S from Rust ([0d3963bc79](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0d3963bc79))
- use Lazy rather than Once for SERVICES ([cd8ba00508](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cd8ba00508))
- build cpu_data.S from Rust ([8961ecdd26](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8961ecdd26))
- save/restore VHE related registers ([1116204fc8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1116204fc8))
- make ENABLE_ASSERTIONS and CRASH_REPORTING follow DEBUG ([5818df298a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5818df298a))
- stop using weak symbols for assembly functions ([392c453ff4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/392c453ff4))
- add generic struct for per-world data ([6c2bb32909](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6c2bb32909))
- allow plat specific cold boot helper ([d77ce2e3c9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d77ce2e3c9))
- remove 8 register message handling ([3a6a4b3217](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3a6a4b3217))
- introduce Cpu trait for CPU specific actions ([a3c01b77fa](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a3c01b77fa))
- use ubfx in get_security_state ([a92fc65c85](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a92fc65c85))

</details>

### Runtime services and world coordination

SMC dispatch is structured around a small service trait and a helper macro to declare ownership of
SMCCC function ID ranges. v0.1.0 includes:

- Arm Architecture calls (SMCCC v1.5 reporting, feature discovery, and architectural workaround
  entry points that delegate to platform hooks when required).
- PSCI (PSCI v1.3 reporting) built on top of the `arm-psci` crate, with an explicit platform power
  topology interface and composite power-state coordination.
- FF-A Secure Partition Manager Dispatcher (SPMD) based on the `arm-ffa` crate, forwarding messages
  between Normal and Secure world.

When built with the `rme` Cargo feature, RF-A also enables Realm world context management and a
minimal RMMD service for EL3 <-> RMM communication, including handling of `RMM_BOOT_COMPLETE`.
(Platform support is constrained: the QEMU platform configuration explicitly rejects RME builds.)

<details>
<summary>Commit list (39)</summary>

- handle SMCs ([f9ba45e648](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f9ba45e648))
- add conversions from arrays to SmcReturn ([d76abd109c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d76abd109c))
- add feature for RME ([0ad7cfcfc0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0ad7cfcfc0))
- implementation of PSCI_FEATURES ([637c3e45a3](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/637c3e45a3))
- stub implementation of SMCCC_ARCH_SOC_ID ([05e154cb0e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/05e154cb0e))
- implement accurate SMC dispatch ([e713548cea](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e713548cea))
- tidy up SMC handling ([5115dc1017](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5115dc1017))
- allow HVCs ([c5e185552e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c5e185552e))
- stub implementations of SMCCC_ARCH_WORKAROUND* ([fdb1537131](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fdb1537131))
- handle FFA_VERSION and one case of FFA_MSG_WAIT ([cdc8c66fc9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cdc8c66fc9))
- fix semantics of psci_features() return value ([1e95ae1b81](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1e95ae1b81))
- setup Realm world for FEAT_RME ([f1b20541f2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f1b20541f2))
- use arm-ffa crate ([cbb8655898](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cbb8655898))
- fix FunctionId conflict in services::owns ([4c28264f26](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4c28264f26))
- handle Realm world in FFA `version` call ([a5b582a4ee](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a5b582a4ee))
- implement CPU suspend, off, on affinity info PSCI functions ([66ab7c4318](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/66ab7c4318))
- implement system off and reset PSCI functions ([c8d87f25b5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c8d87f25b5))
- implement memory protect PSCI functions ([b4b21b3fad](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b4b21b3fad))
- implement function for querying PSCI features ([5d7afa0de9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5d7afa0de9))
- implement CPU freeze PSCI function ([27be5de6a7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/27be5de6a7))
- implement function for querying PSCI node HW state ([5500f129ee](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5500f129ee))
- implement system suspend PSCI function ([01fb932417](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/01fb932417))
- integrate PSCI service ([1b8e7bae86](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1b8e7bae86))
- remove unused PSCI states from percore data ([d3c1319521](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d3c1319521))
- minor cleanups to PSCI service ([5d68c07c3c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5d68c07c3c))
- implement inject_undef64 ([877f9036ef](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/877f9036ef))
- fix psci call before initialization ([94620001e0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/94620001e0))
- minimize items in scope for sel2 feature ([8fdfb37a57](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8fdfb37a57))
- add warm boot entrypoint ([289c4b09e7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/289c4b09e7))
- use different type for MPIDR_EL1 values than PSCI MPIDR ([bf79acfed9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bf79acfed9))
- update arm-ffa crate to 0.2.1 ([e081bee852](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e081bee852))
- handle FFA_VERSION forwarding in STF ([65194ce9b1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/65194ce9b1))
- expected PSCI version in STF ([71675061c3](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/71675061c3))
- better Debug implementation for SmcReturn ([484bfe6d98](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/484bfe6d98))
- add SPMD skeleton ([58b71031ec](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/58b71031ec))
- split secure call handling in SPMD ([45dea8c09b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/45dea8c09b))
- handle FFA interrupt requests in STF ([205567835f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/205567835f))
- do not handle ffa interrupt requests in main bl32 loop ([453a0d55ec](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/453a0d55ec))
- register both ffa handler and test helper ([aab32708e9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/aab32708e9))

</details>

### Platforms

Two reference platforms are supported in this release:

- Arm Base RevC AEM FVP: PL011-based console logging, platform-specific MMIO mappings, and fixed
  entry-point configuration for BL32/BL33 (and manifest/config addresses as currently hard-coded
  constants).
- QEMU `virt`: a PL011 console combined with a per-core in-memory log buffer (hybrid logging), a
  simple multi-core "holding pen" mechanism for bringing up secondaries, and platform-specific MMIO
  mappings.

Platform selection is a compile-time configuration (via `RUSTFLAGS` `--cfg platform=...`) and is
reflected in both the Rust build and in platform-specific linker parameters (BL31 base/size).

<details>
<summary>Commit list (47)</summary>

- add logger using PL011 UART ([70d34eecf2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/70d34eecf2))
- move QEMU-specific memory layout to qemu module ([b9162e4610](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b9162e4610))
- move UART and logger initialisation to platform module ([ef49fb7f00](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ef49fb7f00))
- use config flag rather than feature to choose platform ([6e9bf1fcb6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6e9bf1fcb6))
- implement PSCI_SYSTEM_OFF using semihosting on QEMU ([07dfa5f13b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/07dfa5f13b))
- use pl011-uart crate for PL011 UART driver ([a05e04fadb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a05e04fadb))
- do setup for EL2 GICv3-related registers for FVP ([51324e045d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/51324e045d))
- provide non-secure ep info ([de53ed0e85](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/de53ed0e85))
- switch to arm-pl011-uart and refactor Logger ([0da71ff52d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0da71ff52d))
- initialize GICv3 ([a24bad738c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a24bad738c))
- increase available trusted SRAM on FVP ([6d25898218](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6d25898218))
- update maximal BL31 size on FVP ([6ec361d4df](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6ec361d4df))
- update maximal BL31 size on Qemu ([c239de7cda](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c239de7cda))
- define PSCI platform interface in basic types ([9482f8bee8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9482f8bee8))
- update arm-pl011-uart to 0.3 ([6158e2fd75](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6158e2fd75))
- change base address of bl31 ([fd28672bb0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fd28672bb0))
- increase size of bl31 ([a2a0eb7fcd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a2a0eb7fcd))
- use Once and SpinMutex in GIC init ([b658a822a9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b658a822a9))
- implement Cores on Psci rather than platform ([906540b556](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/906540b556))
- add cpu_standby for qemu ([e50ec46e69](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e50ec46e69))
- switch to latest version of arm-gic ([8938cb8c93](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8938cb8c93))
- use UART1 (secure UART) for RF-A on QEMU ([9b810e8aad](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9b810e8aad))
- add FVP platform to STF ([985b268382](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/985b268382))
- refactor GICv3 initialization ([fe17598681](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fe17598681))
- migrate platform_helpers into Rust ([b61dcb4e11](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b61dcb4e11))
- minimize platform_helpers.S ([4d73953459](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4d73953459))
- fix realm world's SPSR_EL3 value on the test platform ([cbe68345bc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cbe68345bc))
- align SPSR_EL3 values on the test platform ([b5183b1838](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b5183b1838))
- add power_domain_on for qemu ([698bb119e4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/698bb119e4))
- bump arm-gic version to 0.5.0 ([56fd6f1033](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/56fd6f1033))
- remove check for GICv3/4 in crash dump code ([4074f87c46](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4074f87c46))
- add in-memory logger and hybrid logger ([33330a435f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/33330a435f))
- configure timer interrupts for qemu ([4406f8c7f6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4406f8c7f6))
- configure timer interrupts for fvp ([3d9f1536dd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3d9f1536dd))
- correct argument to redistributor ([0bc0c2c6b8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0bc0c2c6b8))
- build PL011 UART crash console driver from Rust ([5a8ab4a54c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5a8ab4a54c))
- qemu: add `power_domain_power_down_wfi` ([3b5d4900e1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3b5d4900e1))
- move PL011 crash console driver to a new module ([733f8939a3](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/733f8939a3))
- build QEMU plat_helpers.S from Rust ([cfe1ea4a84](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cfe1ea4a84))
- bump arm-gic to 0.6.0 ([ab9164b397](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ab9164b397))
- power on redistributors during init ([b139c24b5f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b139c24b5f))
- build FVP arm_helpers.S from Rust ([9b12eafdfc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9b12eafdfc))
- use platform-specific smc handlers ([b9b6467d33](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b9b6467d33))
- qemu: implement disable_cpu_interface ([8038582101](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8038582101))
- hardcode config DT addresses for FVP ([1b8e0f8364](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1b8e0f8364))
- check target CPU state in QEMU and test power_domain_off ([987d450b28](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/987d450b28))
- simplify platform module selection logic ([a26819ac55](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a26819ac55))

</details>

### Build, tooling, and integration testing

RF-A's build system is intentionally pragmatic for this first release: it uses Cargo to build the
Rust BL31 image and integrates with a local TF-A checkout to build BL1/BL2 and generate a FIP.
Convenience `make` targets support running under QEMU or FVP and generating Rustdoc. Release builds
are size-oriented (`opt-level = "s"`, ThinLTO, stripping) and enable Rust integer overflow checks;
both dev and release profiles abort on panic.

The repository includes a Secure Test Framework (STF) workspace member that builds BL32 (SWd) and
BL33 (NWd) test payloads and coordinates tests over FF-A direct messages. v0.1.0 includes test
coverage for PSCI, SMCCC Architecture calls, interrupt behavior, and FF-A/SPMD interactions.

<details>
<summary>Commit list (128)</summary>

- create directory for Rust port of TF-A ([1a5b017204](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1a5b017204))
- add Makefile to run cargo build and objcopy ([6682eb7197](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6682eb7197))
- add Makefile rule to run clippy ([06b5ac00a0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/06b5ac00a0))
- warn clippy::undocumented_unsafe_blocks ([651bedfdfe](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/651bedfdfe))
- split out platform-specific part of linker script ([3fa870f26c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3fa870f26c))
- integrate build systems for QEMU and FVP ([b0057c6a87](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b0057c6a87))
- enable debug builds on rust Makefile ([6ef60d05e3](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6ef60d05e3))
- add platform console definitions for fvp ([dae5aa9d18](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/dae5aa9d18))
- add list_platforms target ([dd4d05dcdc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/dd4d05dcdc))
- Move elf file to target directory ([6846f5baea](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6846f5baea))
- use new Rust 1.82 features ([a5cdf149ba](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a5cdf149ba))
- document how to install dependencies, build and run ([0eb79aad78](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0eb79aad78))
- add run rule for Rust BL31 builds ([2423fb8eae](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2423fb8eae))
- make RUST=1 the default option ([ee63b1f171](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ee63b1f171))
- prevent 'make fvp/qemu' command from triggering builds ([61a21986b1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/61a21986b1))
- add unit test for creating page table ([847816a624](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/847816a624))
- add option to build Rust BL31 with given features ([198f550512](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/198f550512))
- remove unnecessary --target flag ([d7658b4b89](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d7658b4b89))
- use softfloat toolchain ([65c8c67c21](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/65c8c67c21))
- add unit test for SMC handling dispatch ([270a729cb9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/270a729cb9))
- separate BL31 and BL33 builds ([d3e06bd52d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d3e06bd52d))
- add Builders trait for building FVP and Qemu ([b754694969](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b754694969))
- compile TF-A legacy code into libtfa.a ([afe35545e6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/afe35545e6))
- add Rust BL31 and dummy BL33 to FIP ([34e371c9ad](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/34e371c9ad))
- add Rust payloads as a FIP dependency ([cbfcea224d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cbfcea224d))
- have linker scripts for each different build option ([fcb6d3b8f8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fcb6d3b8f8))
- resolve some clippy safety comment warnings ([e3ee36dbbb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e3ee36dbbb))
- fix some warnings for names of statics ([c861d16566](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c861d16566))
- quell unused code warnings ([7e29fdc94a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7e29fdc94a))
- resolve some `cargo clippy` warnings ([b88bb23f87](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b88bb23f87))
- allow clippy::unit_arg in Arch::handle_smc ([2d71652261](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2d71652261))
- mirror FVP's parameters as model is dual cluster ([d05aa493c9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d05aa493c9))
- garden the Makefile ([07c4cb5283](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/07c4cb5283))
- configure cargo vet ([86f3906c5f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/86f3906c5f))
- add audits for crates I maintain ([656676541c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/656676541c))
- audit thiserror and thiserror-impl ([69cd2d37b6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/69cd2d37b6))
- split build.rs main into functions ([b4a53a9194](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b4a53a9194))
- audit `spin` crate ([3754520425](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3754520425))
- vet safe-mmio ([20fb664e9a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/20fb664e9a))
- update and audit `uuid` crate ([a6e3fc3e9f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a6e3fc3e9f))
- audit `num_enum`, `num_enum_derive` ([3267fe1eaf](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3267fe1eaf))
- deny `clippy::undocumented_unsafe_blocks` ([bc02c40e98](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bc02c40e98))
- pre-push hook to check Rust ([cdd14d6ad4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cdd14d6ad4))
- update to Rust 1.85 ([5fefd8c176](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5fefd8c176))
- rebuild RF-A if the linker script(s) changed ([d7312b693b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d7312b693b))
- make 'all' target build everything ([89db93808b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/89db93808b))
- test the `FEATURES` in `pre-push` ([6c581f146c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6c581f146c))
- fix some clippy warnings ([8eb592d5af](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8eb592d5af))
- improve the pre-push script ([9a2bef74a1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9a2bef74a1))
- a script to track object code size ([e67989e08b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e67989e08b))
- add tests for some basic SMCCC arch and PSCI functions ([f0ab0e6a2b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f0ab0e6a2b))
- run some tests initiated from normal world too ([1df22fd472](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1df22fd472))
- stop building TF-A binaries from RF-A repository ([0d8a7fa604](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0d8a7fa604))
- negotiate FF-A version in STF ([cfcca51a41](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cfcca51a41))
- handle exceptions in STF ([08acc32695](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/08acc32695))
- build and run STF from Rust Makefile ([74d758cff2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/74d758cff2))
- audit syn ([5b91c18606](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5b91c18606))
- audit thiserror and thiserror-impl 2.0.12 ([1d3eb76615](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1d3eb76615))
- audit log 0.4.27 ([00a02b3fa4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/00a02b3fa4))
- basic audits for cc and shlex ([cd1d7cdb21](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/cd1d7cdb21))
- exempt cc and shlex from audit ([77e86ef16b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/77e86ef16b))
- audit arm-ffa crate ([d51bba286e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d51bba286e))
- audit arm-psci crate ([eaeb8c4dca](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/eaeb8c4dca))
- audit arm-pl011-uart crate ([095dc9ecb2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/095dc9ecb2))
- provide listing of possible feature combinations ([b92f2fc917](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b92f2fc917))
- delete TF-A build system ([99c3a67759](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/99c3a67759))
- adjust BL31_BASE address for RME builds ([ff19b129e4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/ff19b129e4))
- audit new versions of various crates ([114f450110](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/114f450110))
- audit aarch64-paging ([59d0a9ff85](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/59d0a9ff85))
- audit safe-mmio ([c5ce79cd1a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c5ce79cd1a))
- fix or silence build and clippy warnings ([7cb4e97e76](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7cb4e97e76))
- audit `arm-gic` ([f70bac22b2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f70bac22b2))
- fix warnings for cargo test ([6c4fe9ddbc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6c4fe9ddbc))
- audit new version of arm-ffa ([698e917ce9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/698e917ce9))
- apply cargo fmt to STF ([6b5367f1bc](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6b5367f1bc))
- update STF dependencies and reduce features ([c072be8a3e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c072be8a3e))
- add STF crate to workspace ([af31e5710c](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/af31e5710c))
- test SPM_ID_GET in STF ([db27964647](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/db27964647))
- use NEED_BL31=no for tf-a's build system ([709a4de53a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/709a4de53a))
- let normal-world tests call into secure world ([5db9240c41](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5db9240c41))
- separate out module for parsing test framework messages ([d784e4473a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d784e4473a))
- register tests automatically with a macro ([6c9e756438](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/6c9e756438))
- log errors in test_ffa_spm_id_get ([9ec00729c8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9ec00729c8))
- factor out functions for handling requests ([9d91a52410](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9d91a52410))
- allow normal world tests to specify secure world handler ([5ececa5919](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5ececa5919))
- test forwarding RX_TX_MAP to secure world ([dafe47af08](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/dafe47af08))
- test Normal World to Secure World forwarding ([793500e4de](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/793500e4de))
- move all RF-A files to the top directory ([9edfc219e9](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9edfc219e9))
- fix unused import warning ([266472f207](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/266472f207))
- audit new version of paste ([e9d4c7936d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e9d4c7936d))
- audit linkme and linkme-impl ([93ddf02c20](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/93ddf02c20))
- move test cases and framework into submodules ([bb175400fe](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bb175400fe))
- add heap allocator for STF ([f7707aa232](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f7707aa232))
- sort tests to get consistent order ([83cf6ed217](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/83cf6ed217))
- group tests by topic rather than world ([b5e319d1c5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b5e319d1c5))
- include module path in test names ([102b2fb9f1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/102b2fb9f1))
- test world-switch on timer interrupt ([f2d6965cec](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f2d6965cec))
- reduce level of some STF logs from info to debug ([1429f5cce6](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1429f5cce6))
- configure STF log level via an environment variable ([e72cad3fbd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e72cad3fbd))
- wrap STF console in ExceptionLock ([e5861c938b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e5861c938b))
- use Once for STF logger ([5499f3db3f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/5499f3db3f))
- audit buddy_system_allocator ([fbf8dde42a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fbf8dde42a))
- pass BL31_BASE and BL31_SIZE to common build script ([bdb6c8dd83](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bdb6c8dd83))
- remove unused rt_svc_descs from linker script ([3fa93c4c87](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3fa93c4c87))
- add some Secure World tests (non-forwarding) ([1b0de5eece](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/1b0de5eece))
- check that NS forwarding doesn't happen ([8f1c87d5c1](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8f1c87d5c1))
- check that SW forwarding doesn't happen ([be195ddb21](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/be195ddb21))
- add linker build helpers ([8039f0ccdb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/8039f0ccdb))
- extract PAGE_SIZE to linker symbol ([9a773d5032](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9a773d5032))
- add cargo vet exemptions for zerocopy ([e1140517f4](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e1140517f4))
- audit zerocopy for does-not-implement-crypto ([bbe26df4c2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bbe26df4c2))
- audit autocfg, lock_api and scopeguard as safe-to-run ([0e628f57f8](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0e628f57f8))
- fix cargo test on AArch64 host ([b91528c068](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b91528c068))
- add copyright notice to files missing it ([7f5300059a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7f5300059a))
- fix build warnings ([d93a4b6fc5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d93a4b6fc5))
- fix clippy warnings ([91a4efc470](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/91a4efc470))
- switch from `cargo-objcopy` to `rust-objcopy` ([a47088da69](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a47088da69))
- factor out RUSTFLAGS control ([78cc87c883](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/78cc87c883))
- allow user to configure which cargo to use ([952abc8304](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/952abc8304))
- set STF to use release builds ([bbd103a711](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/bbd103a711))
- fill in license details ([45625409b2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/45625409b2))
- audit arm-gic ([eedb7b2c9f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/eedb7b2c9f))
- audit smccc ([45be62f449](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/45be62f449))
- audit aarch64-rt ([c7c897c322](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c7c897c322))
- audit new versions of arm crates ([404b6b06b5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/404b6b06b5))
- audit new version of thiserror ([474d92aacb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/474d92aacb))
- audit aarch64-paging and arm-gic ([2e76f458a7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/2e76f458a7))
- update arm-* dependencies ([e967846e75](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e967846e75))

</details>

### Documentation

Project documentation includes a style guide, contribution instructions (including DCO
requirements), a getting-started guide for FVP/QEMU, and an RF-A threat model that explicitly builds
on TF-A's firmware and supply-chain threat models while noting the impact of Rust's memory-safety
guarantees.

<details>
<summary>Commit list (19)</summary>

- add comment about SmcFlags matching assembly code ([7da6a027a3](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7da6a027a3))
- update README and Makefile to specify `DEBUG=1` ([fdf774c0a7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/fdf774c0a7))
- document more prerequisites ([e8965b4427](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e8965b4427))
- use DEBUG=1 for debugging too ([7016d04bf5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/7016d04bf5))
- start a code style guide ([3e68910775](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/3e68910775))
- add copyright and `vet` guidance to the style guide ([4cbbf54f5e](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/4cbbf54f5e))
- update instructions to build BL2 with BL32 support ([d1c19b0cf0](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d1c19b0cf0))
- document style for function doc comments ([149c6b95f5](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/149c6b95f5))
- draft beginning of threat model ([9c9766643f](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9c9766643f))
- apply Arm Trademark Guidance ([e191834a5b](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/e191834a5b))
- add option to test rf-a's documentation ([30002902d2](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/30002902d2))
- remove list of Arm trademarks ([f76dad748a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/f76dad748a))
- add clarification on why we build with SPMD_SPM_AT_SEL2=0 ([49a81b65e7](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/49a81b65e7))
- add more documentation for STF ([9d2927ae1d](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/9d2927ae1d))
- document contribution policy ([85bc741d4a](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/85bc741d4a))
- move getting started doc out of README ([d54901e5ce](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/d54901e5ce))
- provide details about communication channels ([a562e01040](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a562e01040))
- add short project introduction to README ([c6d5b92089](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c6d5b92089))
- add instructions on building the documentation ([a339369228](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/a339369228))

</details>

### Misc patches

<details>
<summary>Commit list (7)</summary>

- import Trusted Firmware-A v2.11.0, commit fe4df8bdae0a5d. ([26e37450ca](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/26e37450ca))
- delete all unnecessary TF-A files ([b210916520](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/b210916520))
- remove TF-A's gitignore file ([957f6e5edb](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/957f6e5edb))
- remove TF-A documentation files ([467d3a45fd](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/467d3a45fd))
- remove ReadTheDocs configuration file ([c88ab1ca87](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/c88ab1ca87))
- remove config file for checkpatch.pl ([95b455c456](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/95b455c456))
- remove unused files ([0e9e146282](https://git.trustedfirmware.org/plugins/gitiles/RF-A/rusted-firmware-a/+/0e9e146282))

</details>
