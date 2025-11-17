# Copyright The Rusted Firmware-A Contributors.
#
# SPDX-License-Identifier: BSD-3-Clause
#

ifneq ($(CARGO_TARGET_DIR),)
OUT := $(CARGO_TARGET_DIR)
else
OUT := target
endif

BL31_BIN := $(OUT)/bl31.bin
BL32 := $(OUT)/bl32.bin
BL33 := $(OUT)/bl33.bin
FIP := $(OUT)/fip.bin
BL31_ELF := $(OUT)/bl31.elf

OBJCOPY ?= rust-objcopy

# cargo features to enable. See Cargo.toml for available features.
FEATURES ?= sel2

.PHONY: all cargo-doc clean clippy clippy-test build build-stf list_platforms list_features

PLATFORMS_AVAILABLE := fvp qemu

ifndef PLAT
  ifneq ($(MAKECMDGOALS),$(filter $(MAKECMDGOALS),clean clippy-test help list_platforms))
    $(info error: environment variable PLAT=<xxx> is required. Options are:)
    $(foreach p, $(PLATFORMS_AVAILABLE), $(info * $(p)))
    $(error Please run `make PLAT=...`)
  endif
endif

STF_CARGO_FLAGS := --release
RFA_CARGO_FLAGS := --no-default-features --features "$(FEATURES)"

# Make a release build by default.
DEBUG ?= 0
ifeq ($(DEBUG), 1)
	BUILDTYPE := debug
else
	BUILDTYPE := release
	RFA_CARGO_FLAGS += --release
	FEATURES += max_log_info
endif

TARGET := aarch64-unknown-none-softfloat
CARGO_FLAGS += --target $(TARGET)

TARGET_RUSTFLAGS = --cfg platform=\"${PLAT}\"

# Whether to build core + friends. Primarily needed for special sanitizers or
# optimizations. Requires a nightly Cargo.
BUILD_STD ?= 0

# Whether to enable PAuth and/or BTI support in EL3. Flagged out only because it requires a nightly
# compiler.
PAUTH_EL3 ?= 0
BTI_EL3 ?= 0

ifeq ($(PAUTH_EL3), 1)
	BP_OPTIONS += pac-ret
	TARGET_RUSTFLAGS += --cfg pauth
endif
ifeq ($(BTI_EL3), 1)
	BP_OPTIONS += bti
	TARGET_RUSTFLAGS += --cfg bti
endif
ifneq ($(BP_OPTIONS),)
	BUILD_STD = 1

	empty :=
	space := $(empty) #space
	comma := ,
	TARGET_RUSTFLAGS += -Zbranch-protection=$(subst $(space),$(comma),$(BP_OPTIONS))
endif

ifeq ($(BUILD_STD), 1)
	CARGO_FLAGS += -Zbuild-std=core,compiler_builtins,alloc
	CARGO ?= cargo +nightly
else
	CARGO ?= cargo
endif

TARGET_CARGO := RUSTFLAGS="$(TARGET_RUSTFLAGS) -C target-feature=+vh" $(CARGO)
STF_CARGO := RUSTFLAGS="$(TARGET_RUSTFLAGS) -C link-args=-znostart-stop-gc" $(CARGO)

all: images

build:
	$(TARGET_CARGO) build $(CARGO_FLAGS) $(RFA_CARGO_FLAGS)
	ln -fsr $(OUT)/$(TARGET)/$(BUILDTYPE)/rf-a-bl31 $(BL31_ELF)
	$(OBJCOPY) $(BL31_ELF) -O binary $(BL31_BIN)

build-stf:
	$(STF_CARGO) build --package rf-a-secure-test-framework $(CARGO_FLAGS) $(STF_CARGO_FLAGS)
$(BL32): build-stf
	$(OBJCOPY) $(OUT)/$(TARGET)/release/bl32 -O binary $@
$(BL33): build-stf
	$(OBJCOPY) $(OUT)/$(TARGET)/release/bl33 -O binary $@

clippy-test:
	$(CARGO) clippy --tests --features "$(FEATURES)"

cargo-doc:
	RUSTDOCFLAGS="-D warnings --cfg platform=\"${PLAT}\"" RUSTFLAGS="--cfg platform=\"${PLAT}\"" cargo doc --target $(TARGET) --no-deps  \
	--features "$(FEATURES)"

clippy:
	$(TARGET_CARGO) clippy $(CARGO_FLAGS)

images: $(BL32) $(BL33) build

clean:
	$(CARGO) clean

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
	@echo "  list_features  List all possible FEATURE combinations for the given platform."
	@echo "  list_platforms List all supported platforms."
