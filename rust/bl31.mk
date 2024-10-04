#
# Copyright (c) 2024, Arm Limited and Contributors. All rights reserved.
#
# SPDX-License-Identifier: BSD-3-Clause
#

BL31 := rust/target/bl31.bin
BL33 := rust/target/bl33.bin

.PHONY: $(BL31) $(BL33)
$(BL31):
	${Q}${MAKE} PLAT=${PLAT} DEBUG=${DEBUG} FEATURES=${RUST_FEATURES} -C rust build
$(BL33):
	${Q}${MAKE} PLAT=${PLAT} DEBUG=${DEBUG} FEATURES=${RUST_FEATURES} -C rust build-bl33

all: $(BL33) $(BL31)

FIP_DEPS += $(BL33) $(BL31)
# For RUST builds, TOOL_ADD_IMG is skipped because it performs checks
# about the pre-existence of the bl33.bin file, which we know will be
# generated after the TOOL_ADD_IMG command gets executed.
NEED_BL33 = 0
