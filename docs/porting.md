# Porting guide

A platform or set of related platforms is implemented as a binary crate which depends on the
`rf-a-core` and `rf-a-build` library crates. Reference implementations for the Arm Fixed Virtual Platform
and QEMU are provided in the `rf-a-fvp` and `rf-a-qemu` crates respectively. This document gives an
overview of how to implement your own platform.

## Crate structure

Create a new Rust binary crate for your platform. This can be anywhere, it doesn't need to be in the
same directory as the core RF-A source. If you do want your platform to be in the core RF-A
workspace then some extra configuration is required; this is
[detailed below](#adding-a-platform-to-the-core-rf-a-workspace).

In the `Cargo.toml` of your new crate, add dependencies on the RF-A core crates:

```toml
[dependencies]
rf-a-core = "0.3"

[build-dependencies]
rf-a-build = "0.3"
```

If you want to use an unreleased version of the RF-A core or have local changes, you can use a path
or git dependency rather than specifying a version. You may also want to specify particular features
for `rf-a-core` or disable the default features; for example support for S-EL2 is enabled by
default.

You can also add any other dependencies you need, such as drivers for particular peripherals.

To support build-time configuration of log levels, it is recommended to add features for them:

```toml
[features]
max_log_off = ["rf-a-core/max_log_off"]
max_log_error = ["rf-a-core/max_log_error"]
max_log_warn = ["rf-a-core/max_log_warn"]
max_log_info = ["rf-a-core/max_log_info"]
max_log_debug = ["rf-a-core/max_log_debug"]
max_log_trace = ["rf-a-core/max_log_trace"]
```

It is recommended to enable stricter lints too:

```toml
[lints.clippy]
missing_safety_doc = "deny"
undocumented_unsafe_blocks = "deny"
unreadable_literal = "deny"

[lints.rust]
missing_docs = "deny"
unsafe_op_in_unsafe_fn = "deny"
```

## Build script

Add a `build.rs` build script, calling `rf_a_build::configure_build` on an implementation of
`rf_a_build::Builder` for your platform:

```rust
use rf_a_build::{Builder, configure_build};

fn main() {
    configure_build(&FooBuilder);
}

/// Platform builder implementation for Foo Platform.
pub struct FooBuilder;

impl Builder for FooBuilder {
  // Implement the methods as appropriate for your platform.
}
```

## `Platform` implementation

Your platform is a binary crate, so its entry point is in `src/main.rs`. In this, you need to
provide an implementation of the `Platform` trait:

```rust
use rf_a_core::{cpu_extensions::CpuExtension, gicv3::GicConfig, platform::Platform};

struct Foo;

unsafe impl Platform for Foo {
    type IdMap = IdMap<{ Self::PAGE_HEAP_PAGE_COUNT }>;

    const GIC_CONFIG: GicConfig = GicConfig {
        interrupts_config: &[],
    };

    const CPU_EXTENSIONS: &'static [&'static dyn CpuExtension] = &[];

    // ...
}
```

This trait has a number of constants, types and methods. Refer to the Rustdoc for details of each
one, but as an overview:

- `CORE_COUNT` should be the total number of CPU cores on the system. If different configurations
  are possible and this can vary at runtime, then it should be the highest possible number that are
  supported.
- `GIC_CONFIG` contains a list of interrupts which should be configured by EL3, if any.
- `CPU_EXTENSIONS` contains a list of CPU extensions which should be enabled.
- `secure_entry_point` and `non_secure_entry_point` should return the entry points that RF-A should
  jump to for secure (BL32) and non-secure world (BL33) boot respectively, and the arguments to pass
  in registers x0-x7. If the `rme` feature is enabled then `realm_entry_point` should return the
  entry point for realm world.
- `services` should return a list of any runtime services to enable beyond the mandatory ones.
- `map_extra_regions` is explained in the [memory mapping](#memory-mapping) section below.
- `CrashConsoleImpl` and `LogSinkImpl` are explained in the [logging](#logging) section below.

Once the trait is implemented, you can call some macros to generate the assembly entry point,
platform-dependent static variables and the Rust panic handler:

```rust
use rf_a_core::{all_asm, panic_handler, statics};

statics!(Foo);
all_asm!(Foo);
panic_handler!();
```

## MPIDR parsing

TODO: Explain `mpidr_is_valid` and `core_position`.

## Memory mapping

TODO: Explain `map_extra_regions`, `define_early_mapping!` and `cold_boot_handler` vs.
`init_with_early_mapping` vs. `init`.

## Logging

RF-A supports two kinds of logs: regular runtime logs from Rust code via the `log` macros, and crash
logs from assembly code.

To handle crash logs, you must specify an implementation of the
`rf_a_core::crash_console::CrashConsole` trait for `Platform::CrashConsoleImpl`. For example to
write crash logs to a PL011 UART at base address `UART_BASE` with a 24 MHz clock and 115.2 kb/s
baudrate:

```rust
const UART_CLOCK: u32 = 24_000_000;
const UART_BAUDRATE: u32 = 115_200;

impl Platform for Foo {
    // ...
    type CrashConsoleImpl = Pl011CrashConsole<UART_BASE, UART_CLOCK, UART_BAUDRATE>;
}
```

After a crash the `Platform::panic_handler` method will be called. The default implementation of
this loops forever, but you may override it to do something else such as reboot the system. It must
be implemented as a naked assembly function, as it will be called without a valid stack.

To handle runtime logs, you should call `LOGGER.init(...)` in one of the early-initialisation
methods (`Platform::init_with_early_mapping` or `Platform::init`) with an implementation of
`LogSink`. This type must also be specified as `Platform::LogSinkImpl`. For example, to log to a
PL011 UART:

```rust
use arm_pl011_uart::{Uart, UniqueMmioPointer};
use rf_a_core::{logger::LockedWriter, platform::Platform};

impl Platform for Foo {
    // ...
    type LogSinkImpl = LockedWriter<Uart<'static>>;

    fn init(_arg0: u64, _arg1: u64, _arg2: u64, _arg3: u64) {
        // SAFETY: `UART_BASE_ADDRESS` is the base address of a PL011 UART, and nothing else
        // accesses that address range. The address is valid and identity mapped.
        let uart_registers =
            unsafe { UniqueMmioPointer::new(NonNull::new(UART_BASE_ADDRESS).unwrap()) };
        let mut uart = Uart::new(uart_registers);
        uart.enable(UART_CONFIG, UART_BAUDRATE, UART_CLOCK);
        LOGGER
            .init(LockedWriter::new(uart))
            .expect("Failed to initialise logger");
    }
}
```

A number of implementations of `LogSink` are provided in the `logger` module:

- `LockedWriter` wraps around an implementation of `core::fmt::Write`. Many UART drivers implement
  this, for example.
- `HybridLogger` wraps two other implementations of `LogSink`, with the ability to enable or disable
  one of them at runtime. This could be used for example to log to an in-memory buffer at all times,
  and the UART only during early boot.
- `TimestampedLogger` wraps another implementation of `LogSink` to add a timestamp to each log
  message.
- `MemoryLogger` logs to a single in-memory buffer. It implements `Write` so can be wrapped in a
  `LockedWriter`.
- `PerCoreMemoryLogger` logs to a separate in-memory buffer for each core. Unlike
  `LockedWriter<MemoryLogger>` this doesn't require any locking as there is no contention between
  cores, so is faster.

## Interrupt handling

TODO: Explain `GIC_CONFIG`, `handle_group0_interrupt` and `GIC` initialisation.

## PSCI

TODO: explain `PsciPlatformImpl`.

## CPU operations and errata

TODO: explain `define_cpu_ops!` and `define_errata_list!`.

## Adding a platform to the core RF-A workspace

If you want to include your platform in the core RF-A workspace as a reference platform, then you
additionally need to:

1. Add it to the `workspace.members` field of the top-level `Cargo.toml`.
2. Refer to `rf-a-core` and `rf-a-build` as workspace dependencies rather than by version:

```toml
[dependencies]
rf-a-core = { workspace = true, default-features = false }

[build-dependencies]
rf-a-build = { workspace = true }
```
