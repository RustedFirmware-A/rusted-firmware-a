# Copyright (c) 2023, Google LLC. All rights reserved.
#
# SPDX-License-Identifier: BSD-3-Clause

BL31 := target/bl31.bin

.PHONY: all clean clippy build

all: $(BL31)

build:
	cargo build --target aarch64-unknown-none
$(BL31): build
	cargo objcopy --target aarch64-unknown-none -- -O binary $@

clippy:
	cargo clippy --target aarch64-unknown-none

clean:
	cargo clean
	rm -f target/*.bin
