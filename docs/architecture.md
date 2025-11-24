# RF-A architecture overview

RF-A is implemented as a Rust binary crate in a workspace. As well as RF-A itself the workspace
contains STF (the Secure Testing Framework).

## Modules

Some of the main modules in RF-A are described here.

### Top-level

The top-level module ([`main.rs`]) has the Rust entrypoints:

- `bl31_main` is the Rust entrypoint for the first CPU core on cold boot. This is responsible for
  setting up the pagetable, initialising devices (e.g. the GIC, perhaps a UART, and any other
  platform-specific devices) and the logger, initialising the contexts for lower exception levels
  (ELs) with the appropriate entry points, and then entering the main run loop.
- `psci_warmboot_entrypoint` is the Rust entrypoint for secondary cores when they are turned on, and
  any core on warm boot or wakeup from suspend. This calls into the PSCI module to find the
  appropriate lower EL entry point and update the power domain tree before entering the main
  [run loop](#exceptions).

### `context`

The [`context`] module handles initialising, saving and restoring register context when switching
between EL3 and lower ELs.

This includes:

- Per-world per-CPU context, in `CpuContext` stored in `CPU_STATE`. This includes general-purpose
  registers, lower EL system registers (either EL1 or EL2 depending on whether S-EL2 is enabled) and
  some EL3 system register state. Note that general-purpose registers and a few special system
  registers are saved by the `prepare_el3_entry` function in assembly, because this needs to happen
  before entering the Rust code. The rest of the system registers are saved and restored by Rust
  code.
- Per-world context that is shared across all CPU cores, in `PerWorldContext` stored in
  `PER_WORLD_CONTEXT`. This has a small number of EL3 system registers which affect the operation of
  the lower EL and need to have different values for different worlds, but don't need to be changed
  at runtime. They are initialised by `initialise_per_world_contexts`, possibly modified by enabled
  CPU extensions, and then restored when switching to a different world.
- Per-CPU data, in `CpuData` stored in `PERCPU_DATA`. Currently this only includes the crash buffer,
  which is likely to be refactored in future.

### `cpu`

The [`cpu`] module contains CPU-specific operations. Each CPU model implements the `Cpu` trait,
which includes the `MIDR` register value to identify the CPU and CPU-specific hooks for cold boot,
power down, and dumping system registers on a crash.

Platforms list their CPUs with the `define_cpu_ops!` macro.

### `cpu_extensions`

The [`cpu_extensions`] module contains support for a variety of CPU extensions. For each supported
CPU extension there is an implementation of the `CpuExtension` trait, which includes logic to detect
at runtime whether the extension is present, to configure system registers to enable it if so, and
to save and restore additional context if necessary.

Platforms list the CPU extensions they want to enable in the `Platform::CPU_EXTENSIONS` constant.

### `dram`

The [`dram`] module has some abstractions for storing static variables in different sections of
memory safely.

Platforms generally prefer to keep as much as possible of RF-A in SRAM for security reasons as it is
harder to attack physically, but sometimes due to the limited amount of SRAM available on a platform
it may be necessary to keep some large but less-sensitive data (such as an in-memory logger, which
may have several kilobytes of log buffers per core) in DRAM. These abstractions can be used to
implement this within a platform implementation.

[Two macros are provided](#dram-abstractions): `zeroed_mut!` for objects which can safely
be zero-initialised and will be wrapped in a `SpinMutex` for safe mutable access, and
`lazy_indirect!` for objects which can't be zero-initialised directly so will instead be lazily
initialised on first use.

### `errata_framework`

The [errata framework][`errata_framework`] contains workarounds for CPU and other hardware errata.
Each erratum workaround implements the `Erratum` trait, and then platforms list their relevant
errata with the `define_errata_list!` macro. Errata may be implemented either as submodules of the
`errata_framework` module, or (if they are specific to a particular CPU) in the same module as the
`Cpu` implementation for that CPU.

Depending on the details of the erratum, the workaround may need to be applied either at reset
(whenever a core is turned on, either cold boot or warm boot) or at some other point during runtime
specific to the particular erratum.

Reset errata are called automatically early in the boot path via the `apply_reset_errata` function.
This is implemented in assembly because errata workarounds may need to be applied before any Rust
code runs to ensure correct behaviour.

Runtime errata workarounds are called individually from the appropriate place in the code. The
`erratum_applies` function is provided to check whether a given erratum is both included in the
current platform's list of errata and applies on the current CPU. This should generally be checked
before applying the workaround.

### `exceptions`

The [`exceptions`] module includes code related to switching between EL3 and lower exception levels.
`enter_world` is the key function here, which is used by the main run loop to enter a lower EL via an
ERET after restoring register context. `enter_world` returns with a `RunResult` when there is an
exception from the lower EL, which the run loop then handles appropriately before entering the same
or a different world again.

### `gicv3`

The [`gicv3`] module contains code to initialise and configure the GIC, and to save and restore its state if
necessary when powering cores on and off.

### `logger`

The [`logger`] module contains an implementation of [`log::Log`] wrapping an implementation of the
`LogSink` trait. There are also a number of implementations of `LogSink` provided which platforms
can use to configure logging as they wish. In particular:

- `LockedWriter` wraps an implementation of `core::fmt::Write` in a `SpinMutex`. This can be used to
  share a single UART across all cores, assuming the UART driver implements `Write`.
- `HybridLogger` wraps two `LogSink` implementations and logs to both of them. One of the log sinks
  can be enabled and disabled at runtime. This can be used for example to log both to a UART and to
  an in-memory log buffer, perhaps disabling the UART output once the boot process reaches a certain
  stage while retaining in-memory logging.
- `MemoryLogger` logs to a circular buffer in memory of a configurable size. This doesn't implement
  `LogSink` directly but does implement `core::fmt::Write`, so may be wrapped in a `LockedWriter` to
  use the same buffer for all cores, or in `PerCoreMemoryLogger`.
- `PerCoreMemoryLogger` wraps an instance of `MemoryLogger` for every CPU core. This means that each
  core has its own separate log buffer, and thus avoids the need for locking.

### `pagetable`

The [`pagetable`] module includes constants and functions for managing the EL3 pagetable, based on
the `aarch64-paging` crate. There are two pagetables: an 'early pagetable' (see the
[`early_pagetable`] submodule) set up by assembly code early in the boot process before any Rust code
runs, and a runtime pagetable set up by Rust code shortly after Rust code starts running. This is
necessary because it is not sound to run Rust code without a pagetable (due to atomics having
undefined behaviour when caches are disabled), but managing the full runtime pagetable is too
complicated to manage practically and safely in assembly code.

The early pagetable is specified by the platform via the `define_early_mapping!` macro. This uses a
number of const functions to define a static variable called `EARLY_PAGE_TABLE_RANGES` at build
time containing an encoded form of the pagetable. The `init_early_page_tables` assembly function
then uses this to build the actual early pagetable in memory during cold boot, using the space
reserved for secondary core stacks. This two stage process avoids the memory overhead of including
the full early pagetable in the RF-A binary, while also keeping the assembly code portion relatively
simple.

The runtime pagetable is initialised by `init_runtime_mapping`, which adds mappings for the image
and calls `Platform::map_extra_regions` to add mappings for devices and any other regions specific
to the platform, before switching to the new pagetable. Once this has happened it is safe for
secondary cores to start as the early pagetable is no longer needed.

The runtime pagetable is stored in the `PAGE_TABLE` static variable, wrapped in a `SpinMutex` so
that it can safely be modified at runtime if needed. The root address of the runtime pagetable is
also stored in `PAGE_TABLE_ADDR` so that it can be enabled on secondary cores as soon as they start,
from `bl31_warm_entrypoint`. For the same reason as above this needs to happen from assembly code
before any Rust code runs on the secondary core.

### `platform`

The [`platform`] module contains the `Platform` trait, which is implemented by each platform. Each
supported platform has a submodule under this module, with its `Platform` implementation, some other
platform-specific static variables, and anything else specific to that platform.

### `services`

The [`services`] module contains the `Service` trait which is implemented by each
[runtime service](smc-services.md). These are all grouped together in the `Services` struct, which
has methods to handle dispatching an SMC to the appropriate service.

`Services::run_loop` is the main run loop for RF-A, which runs on each core after initialisation is
complete. This loop essentially calls `enter_world` to enter a particular world at the appropriate
lower EL, handles the `RunResult` (an SMC call, interrupt, or something else which causes an
exception to EL3), switches context if necessary, and repeats.

## Concurrency primitives

As much as possible, RF-A avoids unsafe code. To achieve this, we use a number of safe abstractions
around shared mutable state. Raw `static mut` variables are almost always the wrong solution;
instead one of these abstractions should be used.

### `SpinMutex`

To share mutable state between muliple cores, use [`SpinMutex`] from the `spin` crate. As the name
suggests, this implements a spinlock. It may be used either directly in a `static` variable or
within some other struct. For example, the TRNG service uses a `SpinMutex<EntropyPool>` inside its
service struct to keep track of available entropy shared between all cores.

### `PerCoreState`

Locking a `SpinMutex` has a small cost due to the use of atomic instructions, and may contend with
other cores. To avoid this, in some places where mutable state doesn't need to be accessed from
multiple cores, instead use the `PerCoreState` type. This combines [`PerCore`] and [`ExceptionLock`]
from the [`percore`] crate with [`RefCell`] from the core library to allow safe mutable access to a
separate instance of the contained value for each CPU core in the system. `PerCore` allows code
running on a given CPU core to access only the state associated with that core, while
`ExceptionLock` ensures that exceptions are masked while accessing the state. This is necessary for
soundness, to ensure that an exception doesn't happen while some code is accessing the shared state,
because the exception handler might try to access the same value and find it in an inconsistent
state. In practice RF-A always runs with exceptions masked, so this has no significant cost.

(Note that synchronous exceptions can't be masked, but synchronous exceptions at EL3 are handled in
assembly by `report_unhandled_exception` without calling into any Rust code, so they aren't an issue
here.)

This is used in the [`context`] module to keep the per-core, per-world CPU context. Many CPU
extension modules also use it similarly to store system register context specific to the extension.

### `Once` and `Lazy`

Sometimes it is necessary to store something in a static variable that can't be initialised with a
constant at compile time, either simply because the initialisation expression isn't `const` or
because it needs to be provided by the platform sometime early in the boot process. In these cases,
use the [`Once`] or [`Lazy`] types from the `spin` crate. If the initialisation doesn't depend on
anything else that can't be obtained by calling a function then use `Lazy`, otherwise use `Once`.

### `zeroed_mut!` and `lazy_indirect!` <a name="dram-abstractions"></a>

Some platforms need to store certain large values in a different section of their binary, for
example in a zeroed section of DRAM while the rest of RF-A is in SRAM. The RF-A linker script
includes the `.bss2.dram` section for this purpose. Like `.bss` it will be zeroed by the standard
RF-A cold boot entry point code, but the platform can configure it to use a different section of
memory, separate from the main RF-A image.

To use this safely from Rust two macros are provided.

- `zeroed_mut!` should be used for types which can be initialised as all zeroes. This must be proven
  by them deriving the `zerocopy::FromZeroes` trait. `zeroed_mut!` creates a hidden `static mut`
  variable which can be placed in the desired linker section, along with a `SpinMutex` wrapping a
  mutable reference to it, providing safe mutable access.
- If the type can't safely be initialised as all zeroes, then use `lazy_indirect!` instead. This
  takes an expression which is used with a `Lazy` wrapper to initialise the value. It doesn't
  directly provide mutability, so if you need mutable access then use a `SpinMutex` or
  `PerCoreState` within this.

## Notable dependencies

While we try to keep the dependencies of RF-A minimal, we do use a small number of library crates.
Some of these are maintained under the TrustedFirmware organisation or by RF-A contributors, and
some are third-party crates widely used in the embedded Rust ecosystem. Some of the main library
crates we use are listed below; on top of these there are also a number providing drivers for
specific hardware.

### `aarch64-paging`

This is used for constructing and manipulating page tables according to the AArch64 Virtual Memory
System Architecture.

### `arm-ffa`

This implements common types and parsing for the Arm Firmware Framework for A-profile (FF-A), and so
is used by the FF-A SPMD in RF-A.

### `arm-psci`

This implements common types and parsing for the Arm Power State Coordination Interface (PSCI), and
so is used by the PSCI service implementation in RF-A.

### `arm-sysregs`

This provides functions to read and write AArch64 system registers, and types for accessing their
fields. It is currently maintained within the RF-A repository, but will soon be moved to a separate
repository on the TrustedFirmware Gerrit instance.

### `log`

The [`log`] crate is widely used across the Rust ecosystem, and we likewise use its log macros across
RF-A. We implement a logger which can be configured by each platform to log to a UART, memory or any
other platform-dependent log sink.

### `percore`

This provides safe abstractions around per-core mutable state, which we use primarily for per-core
context which we save and restore for lower ELs.

### `spin`

`spin` provides a number of basic synchronisation primitives based on spinlocks and atomic operations.
We use `SpinMutex`, `Once` and `Lazy` across the codebase for safe shared state across CPU cores and
late initialisation.

[`main.rs`]: ../src/main.rs
[`context`]: ../src/context.rs
[`cpu`]: ../src/cpu.rs
[`cpu_extensions`]: ../src/cpu_extensions.rs
[`dram`]: ../src/dram.rs
[`errata_framework`]: ../src/errata_framework.rs
[`exceptions`]: ../src/exceptions.rs
[`gicv3`]: ../src/gicv3.rs
[`logger`]: ../src/logger.rs
[`pagetable`]: ../src/pagetable.rs
[`early_pagetable`]: ../src/pagetable/early_pagetable.rs
[`platform`]: ../src/platform.rs
[`services`]: ../src/services.rs
[`percore`]: https://crates.io/crates/percore
[`PerCore`]: https://docs.rs/percore/0.2.1/percore/struct.PerCore.html
[`ExceptionLock`]: https://docs.rs/percore/0.2.1/percore/struct.ExceptionLock.html
[`SpinMutex`]: https://docs.rs/spin/latest/spin/mutex/spin/struct.SpinMutex.html
[`spin`]: https://crates.io/crates/spin
[`Once`]: https://docs.rs/spin/latest/spin/type.Once.html
[`Lazy`]: https://docs.rs/spin/latest/spin/type.Lazy.html
[`log`]: https://crates.io/crates/log
[`log::Log`]: https://docs.rs/log/latest/log/trait.Log.html
[`RefCell`]: https://doc.rust-lang.org/stable/core/cell/struct.RefCell.html
