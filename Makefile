# Copyright The Rusted Firmware-A Contributors.
#
# SPDX-License-Identifier: BSD-3-Clause

BL1 := target/bl1.bin
BL2 := target/bl2.bin
BL31_BIN := target/bl31.bin
BL32 := target/bl32.bin
BL33 := target/bl33.bin
FIP := target/fip.bin
BL31_ELF := target/bl31.elf

# cargo features to enable. See Cargo.toml for available features.
FEATURES ?= sel2

.PHONY: all cargo-doc clean clippy clippy-test qemu qemu-build qemu-wait fvp fvp-build build build-stf list_platforms list_features

PLATFORMS_AVAILABLE := fvp qemu
PLAT := $(filter $(PLATFORMS_AVAILABLE), $(MAKECMDGOALS))

ifndef PLAT
  ifneq ($(MAKECMDGOALS),$(filter $(MAKECMDGOALS),clean clippy-test help list_platforms))
    $(info error: environment variable PLAT=<xxx> is required. Options are:)
    $(foreach p, $(PLATFORMS_AVAILABLE), $(info * $(p)))
    $(error Please run `make PLAT=...`)
  endif
endif

TARGET := aarch64-unknown-none-softfloat
CARGO_FLAGS := --target $(TARGET) --no-default-features --features "$(FEATURES)"

# Make a release build by default.
DEBUG ?= 0
ifeq ($(DEBUG), 1)
	BUILDTYPE := debug
else
	BUILDTYPE := release
	CARGO_FLAGS += --release
endif

all: $(PLAT)-build

TFA ?= $(error $$TFA must point to your TF-A source repository)

$(BL1):
	make -C $(TFA) $(TFA_FLAGS) PLAT=$(PLAT) DEBUG=$(DEBUG) bl1
	mkdir -p target
	ln -fsr $(TFA)/build/$(PLAT)/$(BUILDTYPE)/bl1.bin $@

$(BL2):
	make -C $(TFA) $(TFA_FLAGS) PLAT=$(PLAT) DEBUG=$(DEBUG) bl2
	mkdir -p target
	ln -fsr $(TFA)/build/$(PLAT)/$(BUILDTYPE)/bl2.bin $@

$(FIP): $(BL2) build $(BL32) $(BL33)
	make -C $(TFA) $(TFA_FLAGS) PLAT=$(PLAT) DEBUG=$(DEBUG) BL32=$(PWD)/$(BL32) BL33=$(PWD)/$(BL33) fip
	mkdir -p target
	cp $(TFA)/build/$(PLAT)/$(BUILDTYPE)/fip.bin $@
#	Replace existing BL31 image by RF-A into the FIP image.
	$(TFA)/tools/fiptool/fiptool update --soc-fw $(BL31_BIN) $@

build:
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo build $(CARGO_FLAGS)
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo objcopy $(CARGO_FLAGS) -- -O binary $(BL31_BIN)
	ln -fsr target/$(TARGET)/$(BUILDTYPE)/rf-a-bl31 $(BL31_ELF)

build-stf:
	RUSTFLAGS="--cfg platform=\"${PLAT}\" -C link-args=-znostart-stop-gc" cargo build --package rf-a-secure-test-framework --target $(TARGET)
$(BL32): build-stf
	mkdir -p target
	RUSTFLAGS="--cfg platform=\"${PLAT}\" -C link-args=-znostart-stop-gc" cargo objcopy --package rf-a-secure-test-framework --target $(TARGET) --bin bl32 -- -O binary $@
$(BL33): build-stf
	mkdir -p target
	RUSTFLAGS="--cfg platform=\"${PLAT}\" -C link-args=-znostart-stop-gc" cargo objcopy --package rf-a-secure-test-framework --target $(TARGET) --bin bl33 -- -O binary $@

clippy-test:
	cargo clippy --tests --features "$(FEATURES)"

cargo-doc:
	RUSTDOCFLAGS="-D warnings --cfg platform=\"${PLAT}\"" RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo doc --target $(TARGET) --no-deps  \
	--features "$(FEATURES)"

clippy:
	RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo clippy $(CARGO_FLAGS)

QEMU = qemu-system-aarch64
GDB_PORT ?= 1234
QEMU_FLAGS = -machine virt,gic-version=3,secure=on,virtualization=on -cpu max -m 1204M \
	-chardev stdio,signal=off,mux=on,id=char0 -monitor chardev:char0 \
	-serial chardev:char0 -serial chardev:char0 -semihosting-config enable=on,target=native \
	-gdb tcp:localhost:$(GDB_PORT) \
	-display none -bios bl1.bin \
	-smp 4
QEMU_DEPS = $(BL1) $(BL2) $(BL32) $(BL33) build

qemu-build: $(QEMU_DEPS)

qemu: $(QEMU_DEPS)
	cd target && $(QEMU) $(QEMU_FLAGS)

qemu-wait: $(QEMU_DEPS)
	cd target && $(QEMU) $(QEMU_FLAGS) -S

gdb: $(QEMU_DEPS)
	gdb-multiarch target/$(TARGET)/$(BUILDTYPE)/rf-a-bl31 \
		--eval-command="target remote :$(GDB_PORT)"

fvp-build: $(BL1) $(FIP)

fvp: $(BL1) $(FIP)
	FVP_Base_RevC-2xAEMvA \
	  -C cluster0.has_arm_v8-4=1 \
	  -C cluster1.has_arm_v8-4=1 \
	  -C bp.vis.disable_visualisation=1 \
	  -C bp.pl011_uart0.unbuffered_output=1 \
	  -C bp.pl011_uart0.out_file=- \
	  -C bp.terminal_0.start_telnet=0 \
	  -C bp.terminal_1.start_telnet=0 \
	  -C bp.terminal_2.start_telnet=0 \
	  -C bp.terminal_3.start_telnet=0 \
	  -C pctl.startup=0.0.0.0 \
	  -C cluster0.NUM_CORES=4 \
	  -C cluster1.NUM_CORES=4 \
	  -C cluster0.cpu0.semihosting-cwd=target \
	  -C cluster1.cpu0.semihosting-cwd=target \
	  -C bp.secure_memory=1 \
	  -C bp.secureflashloader.fname=$(BL1) \
	  -C bp.flashloader0.fname=$(FIP)

clean:
	cargo clean
	rm -f target/*.bin

list_platforms:
	@echo "${PLATFORMS_AVAILABLE}"

list_features:
# NOTE: If we add even a few more supported configurations, we're going to want a permutation
# function to generate them all.
ifeq (${PLAT}, qemu)
	@echo "''  'sel2'"
else ifeq (${PLAT}, fvp)
	@echo "'' 'sel2' 'rme' 'sel2,rme'"
endif

help:
	@echo "usage: ${MAKE} PLAT=<platform> [VAR=<value> [...]] <target> [...]"
	@echo
	@echo "PLAT is required to specify which platform you wish to build."
	@echo "The available platforms are:"
	@echo
	@echo "  ${PLATFORMS_AVAILABLE}"
	@echo
	@echo "Note that the build system doesn't track dependencies for build"
	@echo "options. Therefore, if any of the build options have changed"
	@echo "since a previous build, a clean build must be performed."
	@echo
	@echo "Supported targets:"
	@echo
	@echo "  all          	Build all binaries for the specified platform."
	@echo "  build       	Build BL31 for the specified platform."
	@echo "  build-stf   	Build the Secure Test Framework."
	@echo "  cargo-doc   	Run `cargo doc` checks for the given platform"
	@echo "  clean        	Clean the build for all platforms."
	@echo "  clippy       	Lint the Rust source tree for the specified platform."
	@echo "  clippy-test 	Lint the Rust source tree for the test configuration."
	@echo "  fvp-build    	Build all binaries for the FVP platform."
	@echo "  fvp          	Run fvp. Target should be invoked after binaries are built."
	@echo "  gdb          	Attach gdb to a running BL31."
	@echo "  list_features  List all possible FEATURE combinations for the given platform."
	@echo "  list_platforms List all supported platforms."
	@echo "  qemu-build   	Build all binaries for the QEMU platform."
	@echo "  qemu         	Run qemu. Target should be invoked after binaries are built."
	@echo "  qemu-wait    	Run qemu and wait for a debugger to be attached. (See gdb.)"
