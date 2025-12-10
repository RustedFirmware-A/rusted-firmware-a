// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

use crate::{
    exceptions::enable_irq_trapping_to_el2,
    platform::{Platform, PlatformImpl},
    util::current_el,
};
use arm_gic::{
    IntId, Trigger,
    gicv3::{GicCpuInterface, GicV3, Group, HIGHEST_NS_PRIORITY, InterruptGroup, SecureIntGroup},
};
use arm_sysregs::read_mpidr_el1;
use log::debug;
use percore::{Cores, ExceptionLock, exception_free};
use spin::{mutex::SpinMutex, once::Once};

// Only private interrupts are supported.
const MAX_INTERRUPT_ID: usize = 31;

type InterruptHandler = fn();

/// To easily access the handler for any private interrupt.
/// Wrapped in ExceptionLock to avoid deadlocks when interrupt fires
/// when the lock is held.
static INTERRUPT_HANDLERS: ExceptionLock<
    SpinMutex<[Option<InterruptHandler>; MAX_INTERRUPT_ID + 1]>,
> = ExceptionLock::new(SpinMutex::new([None; MAX_INTERRUPT_ID + 1]));

static GIC: Once<SpinMutex<GicV3>> = Once::new();
static SECURE: Once<bool> = Once::new();

/// Use linear interrupt id for indexing handler array.
fn get_interrupt_handler_idx(int_id: IntId) -> usize {
    u32::from(int_id) as usize
}

/// Configures the interrupt handler for interrupt with id `int_id`.
///
/// If `callback` is Some(fn), then `fn` will be called between ACK and EOI.
///
/// If callback is None, then any previously registered interrupt handler will be erased.
/// This should be used for clean-up between tests.
pub fn set_interrupt_handler(intid: IntId, trigger: Trigger, callback: Option<InterruptHandler>) {
    if !intid.is_private() {
        panic!("Only private interrupts are supported.");
    }

    let cpu = Some(PlatformImpl::core_index());
    let mut gic = GIC.get().unwrap().lock();

    if callback.is_some() {
        let group = if *SECURE.get().unwrap() {
            Group::Secure(SecureIntGroup::Group1S)
        } else {
            Group::Group1NS
        };

        gic.set_interrupt_priority(intid, cpu, HIGHEST_NS_PRIORITY)
            .unwrap();
        gic.set_group(intid, cpu, group).unwrap();
        gic.set_trigger(intid, cpu, trigger).unwrap();
        gic.enable_interrupt(intid, cpu, true).unwrap();
    } else {
        gic.enable_interrupt(intid, cpu, false).unwrap();
    }

    let idx = get_interrupt_handler_idx(intid);
    exception_free(|token| {
        INTERRUPT_HANDLERS.borrow(token).lock()[idx] = callback;
    });
}

/// Acknowledges the interrupt, calls corresponding handler function and sets EOI.
pub fn handle_group1_interrupt() {
    let int_id = GicCpuInterface::get_and_acknowledge_interrupt(InterruptGroup::Group1).unwrap();
    let core_pos = PlatformImpl::core_position(read_mpidr_el1());
    debug!("Group 1 interrupt {int_id:?} acknowledged on core {core_pos:?}");

    if !int_id.is_private() {
        panic!("Only private interrupts are supported.");
    }

    let idx = get_interrupt_handler_idx(int_id);
    let handler = exception_free(|token| INTERRUPT_HANDLERS.borrow(token).lock()[idx]);

    if let Some(handler_fn) = handler {
        handler_fn();
    } else {
        panic!("No handler registered for interrupt {int_id:?} on core {core_pos:?}");
    }

    GicCpuInterface::end_interrupt(int_id, InterruptGroup::Group1);
    debug!("Group 1 interrupt {int_id:?} EOI on core {core_pos}");
}

/// Enables IRQ handling for the current EL.
pub fn init(secure: bool) {
    // Safety: This is the only place where PlatformImpl::create_gic is called.
    GIC.call_once(|| SpinMutex::new(unsafe { PlatformImpl::create_gic() }));
    SECURE.call_once(|| secure);

    let mut gic = GIC.get().unwrap().lock();

    if secure {
        gic.distributor().enable_group1_secure(true);
    } else {
        gic.distributor().enable_group1_non_secure(true);
    }

    init_core();
}

/// Enables IRQ handling for the current core.
pub fn init_core() {
    if current_el() == 2 {
        enable_irq_trapping_to_el2();
        GicCpuInterface::enable_system_register_el2(true, true);
    } else {
        GicCpuInterface::enable_system_register_el1(true);
    }

    GicCpuInterface::enable_group1(true);
    arm_gic::irq_enable();
}
