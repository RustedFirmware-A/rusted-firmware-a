# Copyright (c) 2023, Google LLC. All rights reserved.
#
# SPDX-License-Identifier: BSD-3-Clause

BL31 := target/bl31.bin

.PHONY: all clean build

all: $(BL31)

build:
	cargo build --target aarch64-unknown-none
$(BL31): build
	cargo objcopy --target aarch64-unknown-none -- -O binary $@

clean:
	cargo clean
	rm -f target/*.bin
