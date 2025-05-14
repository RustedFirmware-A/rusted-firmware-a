/*
 * Copyright (c) 2013-2019, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

.global	plat_panic_handler
.global	plat_handle_el3_ea

	/* -----------------------------------------------------
	 * Endless loop by default.
	 * -----------------------------------------------------
	 */
func plat_panic_handler
	wfi
	b	plat_panic_handler
endfunc plat_panic_handler

	/* -----------------------------------------------------
	 * Platform handler for EL3 External Abort.
	 * -----------------------------------------------------
	 */
func plat_handle_el3_ea
	b	report_unhandled_exception
endfunc plat_handle_el3_ea
