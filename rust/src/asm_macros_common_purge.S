/*
 * Copyright The Rusted Firmware-A Contributors.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

/*
 * This file purges the macros defined in asm_macros_common.S, so that at the end of an `asm!` or
 * `global_asm!` block, we can easily clean up global names we have defined. These 2 files must be
 * kept in sync: every macro we define there must be purged here.
 */

.purgem func
.purgem endfunc
.purgem no_ret
.purgem vector_base
.purgem vector_entry
.purgem end_vector_entry
.purgem synchronize_errors
.purgem exception_return
.purgem _mov_imm16
.purgem mov_imm
.purgem ASM_ASSERT
