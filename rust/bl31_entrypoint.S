/*
 * Copyright (c) 2013-2021, ARM Limited and Contributors. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 */

#include <arch.h>
#include <asm_macros.S>

.global bl31_entrypoint
func bl31_entrypoint
        /* ---------------------------------------------------------------------
         * Stash the previous bootloader arguments x0 - x3 for later use.
         * ---------------------------------------------------------------------
         */
        mov     x20, x0
        mov     x21, x1
        mov     x22, x2
        mov     x23, x3

        adr     x0, runtime_exceptions
        msr     vbar_el3, x0
        isb

        /* TODO: jump to reset_handler. get cpu_ops pointer and apply errata workarounds */

        /* ---------------------------------------------------------------------
         * SCTLR_EL3 has already been initialised - read current value before
         * modifying.
         *
         * SCTLR_EL3.I: Enable the instruction cache.
         *
         * SCTLR_EL3.SA: Enable Stack Alignment check. A SP alignment fault
         *  exception is generated if a load or store instruction executed at
         *  EL3 uses the SP as the base address and the SP is not aligned to a
         *  16-byte boundary.
         *
         * SCTLR_EL3.A: Enable Alignment fault checking. All instructions that
         *  load or store one or more registers have an alignment check that the
         *  address being accessed is aligned to the size of the data element(s)
         *  being accessed.
         * ---------------------------------------------------------------------
         */
        mov     x1, #(SCTLR_I_BIT | SCTLR_A_BIT | SCTLR_SA_BIT)
        mrs     x0, sctlr_el3
        orr     x0, x0, x1
        msr     sctlr_el3, x0
        isb

        bl init_cpu_data_ptr

        /* ---------------------------------------------------------------------
         * Initialise SCR_EL3, setting all fields rather than relying on hw.
         * All fields are architecturally UNKNOWN on reset. The following fields
         * do not change during the TF lifetime. The remaining fields are set to
         * zero here but are updated ahead of transitioning to a lower EL in the
         * function cm_init_context_common().
         *
         * SCR_EL3.TWE: Set to zero so that execution of WFE instructions at
         *  EL2, EL1 and EL0 are not trapped to EL3.
         *
         * SCR_EL3.TWI: Set to zero so that execution of WFI instructions at
         *  EL2, EL1 and EL0 are not trapped to EL3.
         *
         * SCR_EL3.SIF: Set to one to disable instruction fetches from
         *  Non-secure memory.
         *
         * SCR_EL3.SMD: Set to zero to enable SMC calls at EL1 and above, from
         *  both Security states and both Execution states.
         *
         * SCR_EL3.EA: Set to one to route External Aborts and SError Interrupts
         *  to EL3 when executing at any EL.
         * ---------------------------------------------------------------------
         */
        mov_imm x0, ((SCR_RESET_VAL | SCR_EA_BIT | SCR_SIF_BIT) \
                        & ~(SCR_TWE_BIT | SCR_TWI_BIT | SCR_SMD_BIT))
        msr     scr_el3, x0

        /* ---------------------------------------------------------------------
         * Initialise MDCR_EL3, setting all fields rather than relying on hw.
         * Some fields are architecturally UNKNOWN on reset.
         *
         * MDCR_EL3.SDD: Set to one to disable AArch64 Secure self-hosted debug.
         *  Debug exceptions, other than Breakpoint Instruction exceptions, are
         *  disabled from all ELs in Secure state.
         *
         * MDCR_EL3.SPD32: Set to 0b10 to disable AArch32 Secure self-hosted
         *  privileged debug from S-EL1.
         *
         * MDCR_EL3.TDOSA: Set to zero so that EL2 and EL2 System register
         *  access to the powerdown debug registers do not trap to EL3.
         *
         * MDCR_EL3.TDA: Set to zero to allow EL0, EL1 and EL2 access to the
         *  debug registers, other than those registers that are controlled by
         *  MDCR_EL3.TDOSA.
         *
         * MDCR_EL3.TPM: Set to zero so that EL0, EL1, and EL2 System register
         *  accesses to all Performance Monitors registers do not trap to EL3.
         *
         * MDCR_EL3.SCCD: Set to one so that cycle counting by PMCCNTR_EL0 is
         *  prohibited in Secure state. This bit is RES0 in versions of the
         *  architecture with FEAT_PMUv3p5 not implemented, setting it to 1
         *  doesn't have any effect on them.
         *
         * MDCR_EL3.MCCD: Set to one so that cycle counting by PMCCNTR_EL0 is
         *  prohibited in EL3. This bit is RES0 in versions of the
         *  architecture with FEAT_PMUv3p7 not implemented, setting it to 1
         *  doesn't have any effect on them.
         *
         * MDCR_EL3.SPME: Set to zero so that event counting by the programmable
         *  counters PMEVCNTR<n>_EL0 is prohibited in Secure state. If ARMv8.2
         *  Debug is not implemented this bit does not have any effect on the
         *  counters unless there is support for the implementation defined
         *  authentication interface ExternalSecureNoninvasiveDebugEnabled().
         *
         * MDCR_EL3.NSTB, MDCR_EL3.NSTBE: Set to zero so that Trace Buffer
         *  owning security state is Secure state. If FEAT_TRBE is implemented,
         *  accesses to Trace Buffer control registers at EL2 and EL1 in any
         *  security state generates trap exceptions to EL3.
         *  If FEAT_TRBE is not implemented, these bits are RES0.
         *
         * MDCR_EL3.TTRF: Set to one so that access to trace filter control
         *  registers in non-monitor mode generate EL3 trap exception,
         *  unless the access generates a higher priority exception when trace
         *  filter control(FEAT_TRF) is implemented.
         *  When FEAT_TRF is not implemented, this bit is RES0.
         * ---------------------------------------------------------------------
         */
        mov_imm x0, ((MDCR_EL3_RESET_VAL | MDCR_SDD_BIT | \
                      MDCR_SPD32(MDCR_SPD32_DISABLE) | MDCR_SCCD_BIT | \
                      MDCR_MCCD_BIT) & ~(MDCR_SPME_BIT | MDCR_TDOSA_BIT | \
                      MDCR_TDA_BIT | MDCR_TPM_BIT | MDCR_NSTB(MDCR_NSTB_EL1) | \
                      MDCR_NSTBE_BIT | MDCR_TTRF_BIT))

        mrs     x1, id_aa64dfr0_el1
        ubfx    x1, x1, #ID_AA64DFR0_TRACEFILT_SHIFT, #ID_AA64DFR0_TRACEFILT_LENGTH
        cbz     x1, 1f
        orr     x0, x0, #MDCR_TTRF_BIT
1:
        msr     mdcr_el3, x0

        /* ---------------------------------------------------------------------
         * Initialise PMCR_EL0 setting all fields rather than relying
         * on hw. Some fields are architecturally UNKNOWN on reset.
         *
         * PMCR_EL0.LP: Set to one so that event counter overflow, that
         *  is recorded in PMOVSCLR_EL0[0-30], occurs on the increment
         *  that changes PMEVCNTR<n>_EL0[63] from 1 to 0, when ARMv8.5-PMU
         *  is implemented. This bit is RES0 in versions of the architecture
         *  earlier than ARMv8.5, setting it to 1 doesn't have any effect
         *  on them.
         *
         * PMCR_EL0.LC: Set to one so that cycle counter overflow, that
         *  is recorded in PMOVSCLR_EL0[31], occurs on the increment
         *  that changes PMCCNTR_EL0[63] from 1 to 0.
         *
         * PMCR_EL0.DP: Set to one so that the cycle counter,
         *  PMCCNTR_EL0 does not count when event counting is prohibited.
         *
         * PMCR_EL0.X: Set to zero to disable export of events.
         *
         * PMCR_EL0.D: Set to zero so that, when enabled, PMCCNTR_EL0
         *  counts on every clock cycle.
         * ---------------------------------------------------------------------
         */
        mov_imm x0, ((PMCR_EL0_RESET_VAL | PMCR_EL0_LP_BIT | \
                      PMCR_EL0_LC_BIT | PMCR_EL0_DP_BIT) & \
                    ~(PMCR_EL0_X_BIT | PMCR_EL0_D_BIT))

        msr     pmcr_el0, x0
        /* ---------------------------------------------------------------------
         * Enable External Aborts and SError Interrupts now that the exception
         * vectors have been setup.
         * ---------------------------------------------------------------------
         */
        msr     daifclr, #DAIF_ABT_BIT

        /* ---------------------------------------------------------------------
         * Initialise CPTR_EL3, setting all fields rather than relying on hw.
         * All fields are architecturally UNKNOWN on reset.
         *
         * CPTR_EL3.TCPAC: Set to zero so that any accesses to CPACR_EL1,
         *  CPTR_EL2, CPACR, or HCPTR do not trap to EL3.
         *
         * CPTR_EL3.TTA: Set to one so that accesses to the trace system
         *  registers trap to EL3 from all exception levels and security
         *  states when system register trace is implemented.
         *  When system register trace is not implemented, this bit is RES0 and
         *  hence set to zero.
         *
         * CPTR_EL3.TTA: Set to zero so that System register accesses to the
         *  trace registers do not trap to EL3.
         *
         * CPTR_EL3.TFP: Set to zero so that accesses to the V- or Z- registers
         *  by Advanced SIMD, floating-point or SVE instructions (if implemented)
         *  do not trap to EL3.
         *
         * CPTR_EL3.TAM: Set to one so that Activity Monitor access is
         *  trapped to EL3 by default.
         *
         * CPTR_EL3.EZ: Set to zero so that all SVE functionality is trapped
         *  to EL3 by default.
         *
         * CPTR_EL3.ESM: Set to zero so that all SME functionality is trapped
         *  to EL3 by default.
         */

        mov_imm x0, (CPTR_EL3_RESET_VAL & ~(TCPAC_BIT | TTA_BIT | TFP_BIT))
        mrs     x1, id_aa64dfr0_el1
        ubfx    x1, x1, #ID_AA64DFR0_TRACEVER_SHIFT, #ID_AA64DFR0_TRACEVER_LENGTH
        cbz     x1, 1f
        orr     x0, x0, #TTA_BIT
1:
        msr     cptr_el3, x0

        adrp    x0, __BSS_START__
        add     x0, x0, :lo12:__BSS_START__

        adrp    x1, __BSS_END__
        add     x1, x1, :lo12:__BSS_END__
        sub     x1, x1, x0
        bl      zeromem

        /* ---------------------------------------------------------------------
         * Use SP_EL0 for the C runtime stack.
         * ---------------------------------------------------------------------
         */
        msr     spsel, #0
        adrp    x0, boot_stack_end
        add     x0, x0, :lo12:boot_stack_end
        mov     sp, x0

        mov     x0, x20
        mov     x1, x21
        mov     x2, x22
        mov     x3, x23

        bl  bl31_main

endfunc bl31_entrypoint
