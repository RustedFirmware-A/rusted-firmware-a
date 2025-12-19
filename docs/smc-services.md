# SMC services overview

RF-A provides a set of runtime services that software executing at lower exception levels can invoke
via Secure Monitor Calls (SMCs). Each service may be exposed to the secure, normal, or realm world,
or to a combination of these, and its behavior may vary depending on the calling world. If a service
is invoked from a world for which it is not intended, the SMC services framework will typically
report that the service is not supported.

The remainder of this document enumerates the runtime services currently supported by RF-A and
describes each function provided by those services. For services that implement a published
specification, all interfaces defined by the specification are listed, together with an indication
of their level of support in the RF-A implementation.

## Arm Architecture calls (`src/services/arch.rs`)

This service is available to secure, normal and realm worlds.

It implements the Arm Architecture Calls, as documented in the SMCCC specification (Arm document
DEN0028D). It reports the implemented version of the SMC Calling Convention, advertises its
features, provides the SoC identification, and optionally provides CPU vulnerability workarounds.

| Interface                     | Support          | Notes                                                                                               |
| ----------------------------- | ---------------- | --------------------------------------------------------------------------------------------------- |
| `SMCCC_VERSION`               | Supported        | Returns 1.5.                                                                                        |
| `SMCCC_ARCH_FEATURES`         | Supported        | Reports support for version/features/SoC-ID, and for workarounds 1–4 if advertised by the platform. |
| `SMCCC_ARCH_SOC_ID_32/64`     | Supported (stub) | Returns placeholder version/revision and a hard-coded name.                                         |
| `SMCCC_ARCH_WORKAROUND_1/2/3` | Supported        | Executes platform-provided mitigations.                                                             |

## PSCI (`src/services/psci.rs`)

This service is available to normal world only.

It implements the Power State Coordination Interface (PSCI, see Arm document DEN0022), which
provides power management SMCs. Both Platform-Coordinated and OS-Initiated modes (if the platform
opts in) are supported.

| Interface                                 | Support              | Notes                                                                                                                   |
| ----------------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `PSCI_VERSION`                            | Supported            | Returns 1.3.                                                                                                            |
| `CPU_SUSPEND`                             | Supported            |                                                                                                                         |
| `CPU_OFF`                                 | Supported            |                                                                                                                         |
| `CPU_ON`                                  | Supported            | Wakes via `bl31_warm_entrypoint`.                                                                                       |
| `AFFINITY_INFO`                           | Supported            |                                                                                                                         |
| `MIGRATE_INFO_TYPE`                       | Supported            | Always reports `MIGRATION_NOT_REQUIRED` by design: migratable Trusted OS are not supported.                             |
| `MIGRATE` / `MIGRATE_INFO_UP_CPU`         | Will not support     | See `MIGRATE_INFO_TYPE`.                                                                                                |
| `SYSTEM_OFF` / `SYSTEM_RESET`             | Supported            | Calls platform hooks.                                                                                                   |
| `SYSTEM_OFF2` / `SYSTEM_RESET2`           | Platform-gated       |                                                                                                                         |
| `MEM_PROTECT` / `MEM_PROTECT_CHECK_RANGE` | Platform-gated       |                                                                                                                         |
| `PSCI_FEATURES`                           | Supported            | Advertises optional calls according to the platform's features.                                                         |
| `CPU_FREEZE`                              | Platform-gated       |                                                                                                                         |
| `CPU_DEFAULT_SUSPEND`                     | Platform-gated       |                                                                                                                         |
| `NODE_HW_STATE`                           | Platform-gated       |                                                                                                                         |
| `SYSTEM_SUSPEND`                          | Platform-gated       |                                                                                                                         |
| `PSCI_SET_SUSPEND_MODE`                   | Supported            | Allows switching Platform-Coordinated <-> OS-Initiated mode when the latter is supported and state rules are satisfied. |
| `PSCI_STAT_RESIDENCY` / `PSCI_STAT_COUNT` | Not (yet) supported  |                                                                                                                         |

PSCI events are forwarded to Secure partitions (when present) through FF-A SPMD callbacks.

## FF-A SPMD (`src/services/ffa.rs`)

This service is available to secure and normal worlds.

It implements the Secure Partition Manager Dispatcher (SPMD) for FF-A, mediating FF-A calls between
the normal world and the Secure Partition Manager Core (SPMC) and handling secure interrupts plus
PSCI event notifications.

| Interface                                                        | Support              | Notes                                                                                                       |
| ---------------------------------------------------------------- | -------------------- | ----------------------------------------------------------------------------------------------------------- |
| `FFA_VERSION`                                                    | Supported            | Negotiates with SPMC; advertises v1.2 compatibility.                                                        |
| `FFA_FEATURES`                                                   | Supported (limited)  | Limitation: If the call originates from the secure world, returns success without enumerating feature bits. |
| `FFA_RX_ACQUIRE/RELEASE`                                         | Supported            |                                                                                                             |
| `FFA_RXTX_MAP/UNMAP`                                             | Supported            |                                                                                                             |
| `PARTITION_INFO_GET{,_REGS}`                                     | Supported            |                                                                                                             |
| `FFA_ID_GET`                                                     | Supported (limited)  | Limitation: If the calls originates from the non-secure world, returns hard-coded NS endpoint ID.           |
| `FFA_SPM_ID_GET`                                                 | Supported            |                                                                                                             |
| `FFA_CONSOLE_LOG`                                                | Not supported        |                                                                                                             |
| `FFA_MSG_WAIT / FFA_YIELD / FFA_INTERRUPT / FFA_RUN`             | Supported            |                                                                                                             |
| `FFA_NORMAL_WORLD_RESUME`                                        | Supported            | Only accepted during secure interrupt handling to resume Normal World.                                      |
| `FFA_MSG_SEND_DIRECT_REQ/RESP{,2}`                               | Supported            |                                                                                                             |
| `FFA_SECONDARY_EP_REGISTER`                                      | Supported            | Allowed during boot; stores secondary entrypoint for SPMC.                                                  |
| `FFA_NOTIFICATION_*`                                             | Supported            |                                                                                                             |
| `FFA_EL3_INTR_HANDLE`                                            | Not supported        |                                                                                                             |
| Memory sharing/lend/donate/retrieve/reclaim/pause/frag (`MEM_*`) | Supported            |                                                                                                             |

## Errata Management Firmware Interface (`src/services/errata_management.rs`)

This service is available to normal world only.

It implements Arm Errata Management Firmware Interface, as specified by Arm document DEN0100. It
reports the mitigation status for CPU errata known to EL3.

Note that the specification does not prohibit use of this service by other worlds. However, the only
known consumer at present is the OS or hypervisor executing in the normal world. Consequently,
exposure of the service is restricted to that world in order to reduce the attack surface. The
service may be made available to the realm or secure world in the future if required.

| Interface                 | Support       | Notes                                                                                                     |
|---------------------------|---------------|-----------------------------------------------------------------------------------------------------------|
| `EM_VERSION`              | Supported     | Returns 1.0.                                                                                              |
| `EM_FEATURES`             | Supported     | Reports support for `EM_VERSION`, `EM_FEATURES`, `EM_CPU_ERRATUM_FEATURES`.                               |
| `EM_CPU_ERRATUM_FEATURES` | Supported     | Returns mitigation status (unknown, not affected, affected, etc.) based on platform-provided information. |

## Arm True Random Number Generator Firmware Interface (`src/services/trng.rs`)

This service is available to secure, normal and realm worlds.

It implements the TRNG SMCs as defined by Arm document DEN0098.

| Interface                             | Support       | Notes                                                        |
| ------------------------------------- | ------------- | ------------------------------------------------------------ |
| `ARM_TRNG_VERSION`                    | Supported     | Returns v1.0.                                                |
| `ARM_TRNG_FEATURES`                   | Supported     |                                                              |
| `ARM_TRNG_GET_UUID`                   | Supported     | Returns platform UUID (nil UUID indicates TRNG not present). |
| `ARM_TRNG_RND32`                      | Supported     | Generates up to 96 bits of entropy.                          |
| `ARM_TRNG_RND64`                      | Supported     | Generates up to 192 bits of entropy.                         |

## Platform service

Platforms may implement their own SMC service, which can internally further dispatch to sub-services
if needed.

---

_Copyright The Rusted Firmware-A Contributors_
