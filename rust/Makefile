# Copyright (c) 2023, Google LLC. All rights reserved.
#
# SPDX-License-Identifier: BSD-3-Clause

BL1 := target/bl1.bin
BL2 := target/bl2.bin
BL31 := target/bl31.bin
BL33 := target/bl33.bin
FIP := target/fip.bin

.PHONY: all clean clippy qemu fvp build

PLATFORMS_AVAILABLE := fvp qemu
PLAT := $(filter $(PLATFORMS_AVAILABLE), $(MAKECMDGOALS))

ifndef PLAT
  ifneq ($(MAKECMDGOALS),$(filter $(MAKECMDGOALS),help clean))
    $(info error: environment variable PLAT=<xxx> is required. Options are:)
    $(foreach p, $(PLATFORMS_AVAILABLE), $(info * $(p)))
    $(error Please run `make PLAT=...`)
  endif
endif

CARGO_FLAGS=--target aarch64-unknown-none

ifeq ($(DEBUG), 1)
	BUILDTYPE := debug
else
	BUILDTYPE := release
	CARGO_FLAGS += --release
endif

all: $(BL31) $(BL33)

$(BL1): build
	ln -fsr ../build/$(PLAT)/$(BUILDTYPE)/bl1.bin $@
$(BL2): build
	ln -fsr ../build/$(PLAT)/$(BUILDTYPE)/bl2.bin $@
$(BL33): build
	touch $@
$(FIP):
	ln -fsr ../build/$(PLAT)/$(BUILDTYPE)/fip.bin $@

$(BL31):
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo build $(CARGO_FLAGS)
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo objcopy $(CARGO_FLAGS) -- -O binary $@

build: $(BL31)

clippy:
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo clippy --target $(CARGO_FLAGS)

qemu: $(BL1) $(BL2) $(BL31) $(BL33)
	cd target && qemu-system-aarch64 -machine virt,gic-version=3,secure=on -cpu max \
	  -chardev stdio,signal=off,mux=on,id=char0 -monitor chardev:char0 \
	  -serial chardev:char0 -serial chardev:char0 -semihosting-config enable=on,target=native \
	  -display none -bios bl1.bin -s

fvp: $(BL1) $(FIP)
	FVP_Base_RevC-2xAEMvA \
	  -C bp.vis.disable_visualisation=1 \
	  -C bp.pl011_uart0.unbuffered_output=1 \
	  -C bp.pl011_uart0.out_file=- \
	  -C bp.terminal_0.start_telnet=0 \
	  -C bp.terminal_1.start_telnet=0 \
	  -C bp.terminal_2.start_telnet=0 \
	  -C bp.terminal_3.start_telnet=0 \
	  -C pctl.startup=0.0.0.0 \
	  -C cluster0.NUM_CORES=4 \
	  -C cluster0.cpu0.semihosting-cwd=target \
	  -C bp.secure_memory=1 \
	  -C bp.secureflashloader.fname=$(BL1) \
	  -C bp.flashloader0.fname=$(FIP)

clean:
	cargo clean
	rm -f target/*.bin

help:
	@echo "usage: ${MAKE} [PLAT=<platform>] [OPTIONS] [TARGET]"
	@echo ""
	@echo "PLAT is used to specify which platform you wish to build."
	@echo "If no platform is specified, PLAT defaults to: None"
	@echo ""
	@echo "platform = ${PLATFORMS_AVAILABLE}"
	@echo ""
	@echo "Note that the build system doesn't track dependencies for build "
	@echo "options. Therefore, if any of the build options are changed "
	@echo "from a previous build, a clean build must be performed."
	@echo ""
	@echo "Supported Targets:"
	@echo "  all         Build BL31 bootloader binary and create BL33 file"
	@echo "  clean       Clean the build for all platforms"
	@echo "  clippy      Check the coding style of the entire rust source tree"
	@echo "  qemu        Build bootloader binaries for the qemu platform and run qemu"
	@echo "  fvp         Build bootloader binaries for the fvp platform and run fvp"
	@echo "  build       Build BL31 for the specified platform"
	@echo ""
