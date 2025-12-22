# Getting started

Install Rust from [rustup](https://rustup.rs/), then some other tools and the target we need:

```sh
$ sudo apt install clang build-essential lld
$ rustup target add aarch64-unknown-none-softfloat
$ rustup component add llvm-tools
$ cargo install cargo-binutils
```

[Add your SSH public key](https://review.trustedfirmware.org/settings/#SSHKeys) then get the source:

```sh
$ git clone ssh://$TF_USERNAME@review.trustedfirmware.org:29418/RF-A/rusted-firmware-a
$ cd rusted-firmware-a
```

Also fetch the Trusted Firmware-A repository and record its path into the `TFA`
environment variable:

```sh
$ git clone ssh://$TF_USERNAME@review.trustedfirmware.org:29418/TF-A/trusted-firmware-a
$ export TFA=`pwd`/trusted-firmware-a
```

## Getting started with QEMU

Install the required QEMU dependencies:

```sh
$ sudo apt install qemu-system-arm
```

## Build and run in QEMU

Build BL1, BL2 and Rust BL31 and run in QEMU:

```sh
$ PLAT=qemu DEBUG=1 ./build-and-run.sh
```

## Debugging with QEMU

To connect GDB to QEMU:

```sh
$ PLAT=qemu DEBUG=1 QEMU_WAIT=1 ./build-and-run.sh
```

Then, in a separate terminal window, attach `gdb`:

```sh
$ PLAT=qemu DEBUG=1 GDB=1 ./build-and-run.sh
```

If you want QEMU's gdb listener listen on a port other than the default (which
is 1234), specify the GDB_PORT environment variable in both make
invocations:

```sh
$ GDB_PORT=4096 PLAT=qemu DEBUG=1 QEMU_WAIT=1 ./build-and-run.sh
```

In your 2nd terminal, of course:

```sh
$ GDB_PORT=4096 PLAT=qemu DEBUG=1 GDB=1 ./build-and-run.sh
```

(This could be useful if you needed to run many instances of QEMU, such as to
run many tests in parallel.)

## Getting started with FVP

Arm [FVP](https://trustedfirmware-a.readthedocs.io/en/latest/glossary.html#term-FVP)s are complete
simulations of an Arm system, including processor, memory and peripherals. They enable software
development without the need for real hardware.

There exists many types of FVPs.

The rust Makefile is currently using
[FVP_Base_RevC-2xAEMvA](https://git.trustedfirmware.org/plugins/gitiles/ci/tf-a-ci-scripts.git/+/refs/heads/master/model/base-aemv8a.sh)
to run the FVP. Please refer to this [link](https://developer.arm.com/Tools%20and%20Software/Fixed%20Virtual%20Platforms)
to download this or any other FVP.

## Build and run in FVP

### Without RME support

Build C BL1 and BL2, Rust BL31 and FIP, then run everything in FVP:

```sh
$ PLAT=fvp DEBUG=1 ./build-and-run.sh
```

### With RME support

Build C BL1 and BL2 with RME support, Rust BL31 with RME support and FIP, then run everything in FVP:

```sh
$ PLAT=fvp RME=1 DEBUG=1 ./build-and-run.sh
```

## Documentation

See the [RF-A architecture](architecture.md) documentation for an overview of the code structure.

Build the Rustdoc documentation for a given platform:

```sh
make PLAT=<platform> cargo-doc
```

... the built documentation will be found under `target/<target>/doc/rf_a_bl31`.

To display the documentation, open it with your preferred application of choice, for example:

```sh
xdg-open target/aarch64-unknown-none-softfloat/doc/rf_a_bl31/index.html
```
