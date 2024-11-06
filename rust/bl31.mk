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

# TOOL_ADD_PAYLOAD is used instead of the TOOL_ADD_IMG logic because
# the latter performs checks about the pre-existence of the files, which
# we know will be generated after the command gets executed.
$(eval $(call TOOL_ADD_PAYLOAD,$(BL31),--soc-fw,))
$(eval $(call TOOL_ADD_PAYLOAD,$(BL33),--nt-fw,))

# BL33 has already been added by TOOL_ADD_PAYLOAD, so NEED_BL33=0
NEED_BL33 = 0
