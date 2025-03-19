# rf-a-bl31

This directory contains an experimental Rust port of TF-A BL31.

## Getting started

Install Rust from [rustup](https://rustup.rs/), then some other tools and the target we need:

```sh
$ sudo apt install clang build-essential lld
$ rustup target add aarch64-unknown-none-softfloat
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
$ CC=clang make PLAT=qemu RUST=1 DEBUG=1 NEED_BL32=yes
```

Build Rust BL31 and run in QEMU:

```sh
$ cd rust
$ make DEBUG=1 qemu
```

Build and run in QEMU from the top level directory:

```sh
$ make PLAT=qemu RUST=1 run
```

### Debugging with QEMU

To connect GDB to QEMU:

```sh
$ make PLAT=qemu DEBUG=1 qemu-wait
```

Then, in a separate terminal window, attach `gdb`:

```sh
$ make PLAT=qemu DEBUG=1 gdb
```

If you want QEMU's `gdb` listener listen on a port other than the default (which
is 1234), specify the `GDB_PORT` environment variable in both `make`
invocations:

```sh
$ GDB_PORT=4096 make PLAT=qemu DEBUG=1 qemu-wait

# In your 2nd terminal, of course:
$ GDB_PORT=4096 make PLAT=qemu DEBUG=1 gdb
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

### Build and run in FVP

Build and run in FVP from the top level directory:

```sh
$ make PLAT=fvp RUST=1 run
```
