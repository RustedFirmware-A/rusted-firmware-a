// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for interrupt handling and forwarding.

use crate::{
    gicv3::set_interrupt_handler,
    normal_world_test, secure_world_test,
    util::current_el,
    util::timer::{NonSecureTimer, SEL1Timer, SEL2Timer, Timer},
};
use core::{
    hint::spin_loop,
    sync::atomic::{AtomicBool, Ordering},
};
use log::debug;

normal_world_test!(test_timer);
fn test_timer() -> Result<(), ()> {
    test_timer_helper::<NonSecureTimer>()
}

secure_world_test!(test_secure_timer);
fn test_secure_timer() -> Result<(), ()> {
    if current_el() == 2 {
        // TODO: Enable SEL2Timer test for FVP.
        //
        // Right now ACKing a SEL2 interrupt in FVP
        // always returns special value 1023 (means spurious interrupt).
        // It looks like a bug in FVP itself.
        // Enable the test after figuring out what was the issue.
        #[cfg(platform = "fvp")]
        {
            log::warn!("SEL2 timer test skipped!");
            return Ok(());
        }

        test_timer_helper::<SEL2Timer>()
    } else {
        test_timer_helper::<SEL1Timer>()
    }
}

/// A generic helper that runs the timer test logic for any type `TIMER`
/// that implements the `Timer` trait.
fn test_timer_helper<TIMER: Timer>() -> Result<(), ()> {
    // This flag can be safely accessed by both the main loop and an interrupt handler.
    // To be successfully registered as an interrupt handler, a closure cannot take arguments,
    // and therefore cannot capture local environment.
    // For that reason TIMER_HANDLED has to be made static.
    static TIMER_HANDLED: AtomicBool = AtomicBool::new(false);

    // Init again to avoid nasty bugs when this helper is used multiple times.
    TIMER_HANDLED.store(false, Ordering::Release);

    let timer_handler = || {
        debug!("Stopping timer");
        TIMER::stop();
        TIMER_HANDLED.store(true, Ordering::Release);
    };

    // Register the custom handler.
    // The closure can be passed as a function pointer.
    set_interrupt_handler(TIMER::INTERRUPT_ID, Some(timer_handler));

    // Configure the specific timer `TIMER`.
    TIMER::set(1_000_000);

    // Wait until the timer is handled.
    while !TIMER_HANDLED.load(Ordering::Acquire) {
        spin_loop();
    }

    // Clear the handler for timer interrupt.
    set_interrupt_handler(TIMER::INTERRUPT_ID, None);

    Ok(())
}
