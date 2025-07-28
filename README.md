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
$ git clone ssh://$TF_USERNAME@review.trustedfirmware.org:29418/RF-A/rusted-firmware-a
$ cd rusted-firmware-a
```

Also fetch the Trusted Firmware-A repository and record its path into the `TFA`
environment variable:

```sh
$ git clone ssh://$TF_USERNAME@review.trustedfirmware.org:29418/TF-A/trusted-firmware-a
$ export TFA=`pwd`/trusted-firmware-a
```

### Getting started with QEMU

Install the required QEMU dependencies:

```sh
$ sudo apt install qemu-system-arm
```

### Build and run in QEMU

Build C BL1 and BL2 and Rust BL31:

```sh
$ make TFA_FLAGS="CC=clang NEED_BL32=yes NEED_BL31=no" \
    PLAT=qemu DEBUG=1 all
```

Build Rust BL31 and run in QEMU:

```sh
$ make DEBUG=1 qemu
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

### Getting started with FVP

Arm [FVP](https://trustedfirmware-a.readthedocs.io/en/latest/glossary.html#term-FVP)s are complete
simulations of an Arm system, including processor, memory and peripherals. They enable software
development without the need for real hardware.

There exists many types of FVPs.

The rust Makefile is currently using
[FVP_Base_RevC-2xAEMvA](https://git.trustedfirmware.org/plugins/gitiles/ci/tf-a-ci-scripts.git/+/refs/heads/master/model/base-aemv8a.sh)
to run the FVP. Please refer to this [link](https://developer.arm.com/Tools%20and%20Software/Fixed%20Virtual%20Platforms)
to download this or any other FVP.

### Build and run in FVP

#### Without RME support

Build C BL1 and BL2, Rust BL31 and FIP, then run everything in FVP:

```sh
$ make TFA_FLAGS="FVP_TRUSTED_SRAM_SIZE=512 SPD=spmd SPMD_SPM_AT_SEL2=0 NEED_BL31=no" \
    DEBUG=1 fvp
```

**Note 1:** In the above command, the user may notice that we use `SPMD_SPM_AT_SEL2=0` even though
the project is enabling S-EL2 using the default `sel2` feature.
The `rusted-firmware-a` project is currently leveraging on the `trusted-firmware-a` project's build
system and the latter requires a SP layout file for building with `SPMD_SPM_AT_SEL2=1`. We currently
use the temporary workaround of building with `SPMD_SPM_AT_SEL2=0` to avoid using this sp layout
file.

**Note 2:** By default, TF-A considers that the Base FVP platform has 256 kB of Trusted SRAM.
Actually it can simulate up to 512 kB of Trusted SRAM, which is the configuration we use for RF-A
(because a debug build of RF-A is too big to fit in 256 kB). The `FVP_TRUSTED_SRAM_SIZE=512` TF-A
build flag is required to stop TF-A from complaining that RF-A does not fit.

#### With RME support

Build C BL1 and BL2 with RME support, Rust BL31 with RME support and FIP:

```sh
$ make TFA_FLAGS="FVP_TRUSTED_SRAM_SIZE=512 ENABLE_RME=1 NEED_BL31=no" \
    FEATURES=rme DEBUG=1 fvp
```

Running the FVP with RME through RF-A build system is not supported at this time.

## License

The project is provided under the BSD-3-Clause license, see [LICENSE][3]. Contributions to this
project are accepted under the same license and must also be made under the terms of the
[Developer Certificate of Origin][4], confirming that the code submitted can (legally) become part
of the project.

## Arm trademark notice

Arm is a registered trademark of Arm Limited (or its subsidiaries or affiliates).

This project uses some of the Arm product, service or technology trademarks, as listed in the
[Trademark List][1], in accordance with the [Arm Trademark Use Guidelines][2].

Subsequent uses of these trademarks throughout this repository do not need to be prefixed with the
Arm word trademark.

[1]: https://www.arm.com/company/policies/trademarks/arm-trademark-list
[2]: https://www.arm.com/company/policies/trademarks/guidelines-trademarks
[3]: ./LICENSE
[4]: https://developercertificate.org

---

_Copyright The Rusted Firmware-A Contributors_
