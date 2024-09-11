# Copyright (c) 2023, Google LLC. All rights reserved.
#
# SPDX-License-Identifier: BSD-3-Clause

BL31 := target/bl31.bin

.PHONY: all clean clippy build

all: $(BL31)

build:
	RUSTFLAGS="--cfg platform=\"qemu\"" cargo build --target aarch64-unknown-none
$(BL31): build
	RUSTFLAGS="--cfg platform=\"qemu\"" cargo objcopy --target aarch64-unknown-none -- -O binary $@

clippy:
	RUSTFLAGS="--cfg platform=\"qemu\"" cargo clippy --target aarch64-unknown-none

clean:
	cargo clean
	rm -f target/*.bin
