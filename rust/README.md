# rf-a-bl31

This directory contains an experimental Rust port of TF-A BL31.

## Getting started

Install Rust from [rustup](https://rustup.rs/), then some other tools and the target we need:

```sh
$ rustup target add aarch64-unknown-none
$ rustup component add llvm-tools
$ cargo install cargo-binutils
```

[Add your SSH public key](https://review.trustedfirmware.org/settings/#SSHKeys) then get the source:

```sh
$ git clone ssh://$TF_USERNAME@review.trustedfirmware.org:29418/TF-A/trusted-firmware-a
$ cd trusted-firmware-a
$ git checkout tfa-next
```

### Getting started with QEMU

Install the required QEMU dependencies:

```sh
$ sudo apt install qemu-system-arm
```

### Build and run in QEMU

Build C BL1 and BL2 and Rust BL31:

```sh
$ CC=clang make PLAT=qemu RUST=1 DEBUG=1
```

Build Rust BL31 and run in QEMU:

```sh
$ cd rust
$ make DEBUG=1 qemu
```

Build and run in QEMU from the top level directory:

```
$ make PLAT=qemu RUST=1 run
```

### Debugging with QEMU

To connect GDB to QEMU:

```
$ gdb-multiarch target/aarch64-unknown-none/debug/rf-a-bl31
(gdb) target remote :1234
```

To make QEMU wait for GDB, add `-S` to the end of the QEMU command-line in the `Makefile`.

## Getting started with FVP

Arm [FVP](https://trustedfirmware-a.readthedocs.io/en/latest/glossary.html#term-FVP)s are complete
simulations of an Arm system, including processor, memory and peripherals. They enable software
development without the need for real hardware.

There exists many types of FVPs.

The rust Makefile is currently using
[FVP_Base_RevC-2xAEMvA](https://git.trustedfirmware.org/plugins/gitiles/ci/tf-a-ci-scripts.git/+/refs/heads/master/model/base-aemv8a.sh)
to run the FVP. Please refer to this [link](https://developer.arm.com/Tools%20and%20Software/Fixed%20Virtual%20Platforms)
to download this or any other FVP.

### Build and run in FVP

Build and run in FVP from the top level directory:

```
$ make PLAT=fvp RUST=1 run
```
