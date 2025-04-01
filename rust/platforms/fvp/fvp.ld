/*
 * Copyright (c) 2024, Arm Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

FVP_BL2_BASE = 0x4060000;
FVP_BL31_BASE = 0x4003000;

MEMORY
{
	image : ORIGIN = FVP_BL31_BASE, LENGTH = (FVP_BL2_BASE - FVP_BL31_BASE)
}
