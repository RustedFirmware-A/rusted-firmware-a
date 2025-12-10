#!/bin/bash

# Copyright The Rusted Firmware-A Contributors.
#
# SPDX-License-Identifier: BSD-3-Clause

set -e

if [ -z ${TFA} ]; then
    echo "error: environment variable TFA=<xxx> is required."
    echo "error: Please run TFA=<path/to/trusted-firmware-a> $(basename $0)"
    exit 1
fi

if [ -z ${DEBUG} ]; then
    BUILDTYPE=release
else
    BUILDTYPE=debug
    DEBUG="DEBUG=${DEBUG}"
fi

if [ -z ${PAUTH_EL3} ]; then
    PAUTH_EL3=0
fi

if [ -z ${PAUTH_LR_EL3} ]; then
    PAUTH_LR_EL3=0
fi

if [ -z ${BTI_EL3} ]; then
    BTI_EL3=0
fi

if [ -z "${CARGO}" ]; then
    CARGO="cargo"
fi

if [ -z "${CARGO_TARGET_DIR}" ]; then
    OUT=${PWD}/target
else
    OUT=${CARGO_TARGET_DIR}
fi

TARGET=aarch64-unknown-none-softfloat

BL1=${TFA}/build/${PLAT}/${BUILDTYPE}/bl1.bin
BL2=${TFA}/build/${PLAT}/${BUILDTYPE}/bl2.bin
BL32="${BL32:-${OUT}/bl32.bin}"
BL33="${BL33:-${OUT}/bl33.bin}"
FIP=${TFA}/build/${PLAT}/${BUILDTYPE}/fip.bin

CURRDIR=$(readlink -f "$(dirname "$0")")
pushd $CURRDIR

QEMU=${QEMU:-qemu-system-aarch64}
GDB_PORT=${GDB_PORT:-1234}
QEMU_FLAGS=${QEMU_FLAGS:--machine virt,gic-version=3,secure=on,virtualization=on -cpu max -m 1204M \
	-chardev stdio,signal=off,mux=on,id=char0 -monitor chardev:char0 \
	-serial chardev:char0 -serial chardev:char0 -semihosting-config enable=on,target=native \
	-gdb tcp:localhost:${GDB_PORT} \
	-display none -bios ${BL1} \
	-smp 4}

case "$PLAT" in
  qemu)
    if [[ "${QEMU_WAIT}" == 1 ]]; then
        QEMU_WAIT="-S"
    fi
    make -C $TFA PLAT=qemu ${DEBUG} CC=clang NEED_BL32=yes NEED_BL31=no bl1 bl2
    make PLAT=qemu ${DEBUG} CARGO="${CARGO}" PAUTH_EL3=${PAUTH_EL3} PAUTH_LR_EL3=${PAUTH_LR_EL3} BTI_EL3=${BTI_EL3} all
    ln -fsr ${BL1} ${OUT}
    ln -fsr ${BL2} ${OUT}
    cd ${OUT}
    if [[ "${GDB}" == 1 ]]; then
        gdb-multiarch ${OUT}/${TARGET}/${BUILDTYPE}/rf-a-bl31 --eval-command="target remote :${GDB_PORT}"
    else
        ${QEMU} ${QEMU_FLAGS} ${QEMU_WAIT}
    fi
    ;;

  fvp)
    FVP_COMMON_ARGS="-C bp.terminal_0.start_telnet=0 \
        -C bp.terminal_1.start_telnet=0 \
        -C bp.terminal_2.start_telnet=0 \
        -C bp.terminal_3.start_telnet=0 \
        -C bp.pl011_uart0.unbuffered_output=1 \
        -C bp.pl011_uart0.out_file=- \
        -C bp.pl011_uart1.unbuffered_output=1 \
        -C bp.pl011_uart1.out_file=- \
        -C bp.ve_sysregs.exit_on_shutdown=1 \
        -C bp.vis.disable_visualisation=1 \
        -C cache_state_modelled=1 \
        -C cci550.force_on_from_start=1 \
        -C cluster0.NUM_CORES=4 \
        -C cluster1.NUM_CORES=4 \
        -C cluster0.cpu0.etm-present=0 \
        -C cluster0.cpu1.etm-present=0 \
        -C cluster0.cpu2.etm-present=0 \
        -C cluster0.cpu3.etm-present=0 \
        -C cluster1.cpu0.etm-present=0 \
        -C cluster1.cpu1.etm-present=0 \
        -C cluster1.cpu2.etm-present=0 \
        -C cluster1.cpu3.etm-present=0 \
        -C cluster0.cpu0.semihosting-cwd=${OUT} \
        -C cluster1.cpu0.semihosting-cwd=${OUT} \
        -C cluster0.has_amu=1 \
        -C cluster1.has_amu=1 \
        -C cluster0.has_branch_target_exception=1 \
        -C cluster1.has_branch_target_exception=1 \
        -C cluster0.has_ete=1 \
        -C cluster1.has_ete=1 \
        -C cluster0.has_fgt2=2 \
        -C cluster1.has_fgt2=2 \
        -C cluster0.has_mpam=2 \
        -C cluster1.has_mpam=2 \
        -C cluster0.has_self_hosted_trace_extension=2 \
        -C cluster1.has_self_hosted_trace_extension=2 \
        -C cluster0.has_trbe=1 \
        -C cluster1.has_trbe=1 \
        -C cluster0.has_v8_7_pmu_extension=2 \
        -C cluster1.has_v8_7_pmu_extension=2 \
        -C cluster0.memory_tagging_support_level=2 \
        -C cluster1.memory_tagging_support_level=2 \
        -C cluster0.gicv3.extended-interrupt-range-support=1 \
        -C cluster1.gicv3.extended-interrupt-range-support=1 \
        -C gic_distributor.ARE-fixed-to-one=1 \
        -C gic_distributor.extended-ppi-count=64 \
        -C gic_distributor.extended-spi-count=1024 \
        -C pctl.startup=0.0.0.0 \
        -C bp.secureflashloader.fname=${BL1} \
        -C bp.flashloader0.fname=${FIP}"

    if [ -z ${SP_LAYOUT_FILE} ]; then
        SPMD_SPM_AT_SEL2=${SPMD_SPM_AT_SEL2:-0}
    else
        SPMD_SPM_AT_SEL2=1
    fi

    # Note: By default, TF-A considers that the Base FVP platform has 256 kB of Trusted SRAM.
    # Actually it can simulate up to 512 kB of Trusted SRAM, which is the configuration we use for RF-A
    # (because a debug build of RF-A is too big to fit in 256 kB). The `FVP_TRUSTED_SRAM_SIZE=512` TF-A
    # build flag is required to stop TF-A from complaining that RF-A does not fit.
    if [[ "${RME:-}" == 1 ]]; then
        make PLAT=fvp FEATURES=sel2,rme ${DEBUG} CARGO="${CARGO}" PAUTH_EL3=${PAUTH_EL3} PAUTH_LR_EL3=${PAUTH_LR_EL3} BTI_EL3=${BTI_EL3} all
        make -C $TFA PLAT=fvp ${DEBUG} FVP_TRUSTED_SRAM_SIZE=512 ENABLE_RME=1 \
            BL31=${OUT}/bl31.bin \
            BL32=${BL32} \
            BL33=${BL33} \
            all fip
        FVP_Base_RevC-2xAEMvA \
            -C bp.secure_memory=0 \
            -Q 1000 \
            -C bp.refcounter.non_arch_start_at_default=1 \
            -C bp.has_rme=1 \
            -C bp.dram_metadata.is_enabled=1 \
            -C bp.ls64_testing_fifo.op_type=0 \
            -C cluster0.restriction_on_speculative_execution=2 \
            -C cluster0.restriction_on_speculative_execution_aarch32=2 \
            -C cluster0.stage12_tlb_size=1024 \
            -C cluster0.check_memory_attributes=0 \
            -C pci.pci_smmuv3.mmu.SMMU_AIDR=2 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR1=0x00600002 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR3=0x1714 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR1=0xA0000002 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR2=0 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR3=0 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR0=0x4046123b \
            -C pci.pci_smmuv3.mmu.SMMU_IDR5=0xFFFF0475 \
            -C pci.pci_smmuv3.mmu.SMMU_ROOT_IDR0=3 \
            -C pci.pci_smmuv3.mmu.SMMU_ROOT_IIDR=0x43B \
            -C pci.pci_smmuv3.mmu.root_register_page_offset=0x20000 \
            -C cluster0.has_arm_v9-2=1 \
            -C cluster1.has_arm_v9-2=1 \
            -C cluster0.rme_support_level=2 \
            -C cluster0.gicv3.cpuintf-mmap-access-level=2 \
            -C cluster0.gicv4.mask-virtual-interrupt=1 \
            -C cluster0.gicv3.without-DS-support=1 \
            -C cluster0.max_32bit_el=-1 \
            -C cluster0.PA_SIZE=48 \
            -C cluster0.output_attributes=ExtendedID[62:55]=MPAM_PMG,ExtendedID[54:39]=MPAM_PARTID,ExtendedID[38:37]=MPAM_SP \
            -C cluster0.has_rndr=1 \
            -C cluster0.arm_v8_7_accelerator_support_level="" \
            -C cluster1.restriction_on_speculative_execution=2 \
            -C cluster1.restriction_on_speculative_execution_aarch32=2 \
            -C cluster1.stage12_tlb_size=1024 \
            -C cluster1.check_memory_attributes=0 \
            -C cluster1.rme_support_level=2 \
            -C cluster1.gicv3.cpuintf-mmap-access-level=2 \
            -C cluster1.gicv4.mask-virtual-interrupt=1 \
            -C cluster1.gicv3.without-DS-support=1 \
            -C cluster1.max_32bit_el=-1 \
            -C cluster1.PA_SIZE=48 \
            -C cluster1.output_attributes=ExtendedID[62:55]=MPAM_PMG,ExtendedID[54:39]=MPAM_PARTID,ExtendedID[38:37]=MPAM_SP \
            -C cluster1.has_rndr=1 \
            -C cluster1.arm_v8_7_accelerator_support_level="" \
            ${FVP_COMMON_ARGS}

    else
        make PLAT=fvp ${DEBUG} CARGO="${CARGO}" PAUTH_EL3=${PAUTH_EL3} PAUTH_LR_EL3=${PAUTH_LR_EL3} BTI_EL3=${BTI_EL3} all
        make -C $TFA PLAT=fvp ${DEBUG} FVP_TRUSTED_SRAM_SIZE=512 SPD=spmd SPMD_SPM_AT_SEL2=${SPMD_SPM_AT_SEL2} CTX_INCLUDE_AARCH32_REGS=0 \
            BL31=${OUT}/bl31.bin \
            BL32=${BL32} \
            BL33=${BL33} \
            SP_LAYOUT_FILE=${SP_LAYOUT_FILE} \
            all fip
        FVP_Base_RevC-2xAEMvA \
            -C cluster0.has_arm_v9-0=1 \
            -C cluster1.has_arm_v9-0=1 \
            -C bp.dram_metadata.is_enabled=1 \
            -C pci.pci_smmuv3.mmu.SMMU_AIDR=0x2 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR0=0x0046123B \
            -C pci.pci_smmuv3.mmu.SMMU_IDR1=0x00600002 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR3=0x1714 \
            -C pci.pci_smmuv3.mmu.SMMU_IDR5=0xFFFF0472 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR1=0xA0000002 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR2=0 \
            -C pci.pci_smmuv3.mmu.SMMU_S_IDR3=0 \
            -C pci.dma330x4.use_smmuv3testengine_not_dmacs=1 \
            -C bp.secure_memory=1 \
            ${FVP_COMMON_ARGS}
    fi
    ;;

  *)
    echo "PLAT '${PLAT}' is not supported by this script."
    popd
    exit 1
    ;;
esac

popd
