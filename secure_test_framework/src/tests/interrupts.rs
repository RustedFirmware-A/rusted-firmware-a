// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Tests for interrupt handling and forwarding.

use crate::{
    framework::{
        TestHelperProxy, TestHelperRequest, TestHelperResponse, TestResult, normal_world_test,
        secure_world_test,
    },
    gicv3::set_interrupt_handler,
    util::{
        current_el,
        timer::{NonSecureTimer, SEL1Timer, SEL2Timer, Timer},
    },
};
use arm_ffa::Interface;
use arm_gic::{Trigger, wfi};
use core::sync::atomic::{AtomicBool, Ordering};
use log::debug;

/// Generic response to just indicate that the secure world helper
/// phase has been executed successfully.
const PHASE_SUCCESS: TestHelperResponse = [0, 0, 0, 0];

/// Phase of a timer test to be executed in TestHelperProxy.
#[repr(u64)]
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum Phase {
    /// Setup timer and interrupt handler.
    Setup = 0,
    /// Check if the timer interrupt has been handled.
    CheckInterruptStatus = 1,
    /// Clear the registered interrupt handler.
    Cleanup = 2,
}

impl TryFrom<u64> for Phase {
    type Error = ();

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Phase::Setup),
            1 => Ok(Phase::CheckInterruptStatus),
            2 => Ok(Phase::Cleanup),
            _ => Err(()),
        }
    }
}

/// Builds a TestHelperRequest to run given secure world test phase.
fn helper_run_phase_request(phase: Phase) -> TestHelperRequest {
    [phase as u64, 0, 0]
}

/// Builds a TestHelperResponse to inform the non-secure world
/// if the timer interrupt has been handled.
fn helper_timer_interrupt_status_response(timer_handled: bool) -> TestHelperResponse {
    [timer_handled as u64, 0, 0, 0]
}

/// Common helper for all timer tests.
///
/// Single-world tests will call this helper
/// in the same world as `timer_test_main_loop`.
///
/// On the other hand, the world switch test calls:
/// - the main loop in the non-secure world,
/// - this helper in the secure world.
/// (World switch happens when TestHelperProxy is called).
fn timer_helper<TIMER: Timer>(request: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    let [phase, ..] = request;

    // This flag can be safely accessed by both the main loop and an interrupt handler.
    // To be successfully registered as an interrupt handler, a closure cannot take arguments,
    // and therefore cannot capture local environment.
    // For that reason TIMER_HANDLED has to be made static.
    static TIMER_HANDLED: AtomicBool = AtomicBool::new(false);

    let setup_phase = || {
        // Init again as the static atomic is reused across tests.
        TIMER_HANDLED.store(false, Ordering::Release);

        let timer_handler = || {
            debug!("Stopping timer");
            TIMER::stop();
            TIMER_HANDLED.store(true, Ordering::Release);
        };

        // Register the custom handler.
        // The closure can be passed as a function pointer.
        set_interrupt_handler(TIMER::INTERRUPT_ID, Trigger::Level, Some(timer_handler));

        // Configure the specific timer `TIMER`.
        TIMER::set(1_000_000);

        Ok(PHASE_SUCCESS)
    };

    let check_if_timer_handled_phase = || {
        let timer_handled = TIMER_HANDLED.load(Ordering::Acquire);

        Ok(helper_timer_interrupt_status_response(timer_handled))
    };

    let cleanup_phase = || {
        // Clear the handler for timer interrupt.
        set_interrupt_handler(TIMER::INTERRUPT_ID, Trigger::Level, None);

        Ok(PHASE_SUCCESS)
    };

    // Execute part of the helper that corresponds to current phase of the test.
    match Phase::try_from(phase)? {
        Phase::Setup => setup_phase(),
        Phase::CheckInterruptStatus => check_if_timer_handled_phase(),
        Phase::Cleanup => cleanup_phase(),
    }
}

/// Calls the generic `timer_helper` with `NonSecureTimer` timer implementation
fn nonsecure_timer_helper(ns_world_request: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    timer_helper::<NonSecureTimer>(ns_world_request)
}

/// Calls the generic `timer_helper` with appropriate secure world timer implementation.
fn secure_timer_helper(ns_world_request: TestHelperRequest) -> Result<TestHelperResponse, ()> {
    if current_el() == 2 {
        timer_helper::<SEL2Timer>(ns_world_request)
    } else {
        timer_helper::<SEL1Timer>(ns_world_request)
    }
}

/// Orchestrates the timer test.
///
/// It is necessary for the both-world test,
/// but single-world tests use it as well to reuse the code.
///
/// If timer interrupt handling (or world switching) works incorrectly
/// then the tests below will hang.
fn timer_test_main_loop(helper: &TestHelperProxy) -> TestResult {
    // Setup timer test.
    helper(helper_run_phase_request(Phase::Setup))?;

    loop {
        // Check if the timer interrupt has been handled.
        let [timer_handled, ..] = helper(helper_run_phase_request(Phase::CheckInterruptStatus))?;

        if timer_handled == 1 {
            // Timer has been handled.
            // We can finish the test now.
            break;
        }

        // Wait until the core receives an interrupt.
        wfi();
    }

    // Clear the registered interrupt handler.
    helper(helper_run_phase_request(Phase::Cleanup))?;

    Ok(())
}

/// The world switch test expects that the secure world will receive an Interface::Interrupt
/// request when the secure timer interrupt is routed to the secure world.
///
/// Main advantage of handling this ffa request in the custom test-specific handler
/// is that if some other test does not expect any interrupt ffa requests, then the lack
/// of global handler will cause an error when an unexpected interrupt happens.
fn ffa_interrupt_request_handler(interface: Interface) -> Option<Interface> {
    let Interface::Interrupt { .. } = interface else {
        return None;
    };

    // The interrupt should have already been handled asynchronously at this point.
    Some(Interface::NormalWorldResume)
}

normal_world_test!(
    test_timer_interrupt_world_switch,
    helper = secure_timer_helper,
    // Handles additional FFA requests expected by this test
    // to happen which are not handled in secure world main loop.
    handler = ffa_interrupt_request_handler
);
/// Tests if a secure interrupt is routed to the secure world
/// when the core is idle (wfi) in the non secure world.
fn test_timer_interrupt_world_switch(helper: &TestHelperProxy) -> TestResult {
    timer_test_main_loop(helper)
}

normal_world_test!(test_nonsecure_timer);
/// Uses the same utils as the world switch test above,
/// but the entire test runs only in the non-secure world.
///
/// This just tests the normal-world interrupt handling
/// without world-switching.
fn test_nonsecure_timer() -> TestResult {
    timer_test_main_loop(&nonsecure_timer_helper)
}

secure_world_test!(test_secure_timer);
/// Uses the same utils as the world switch test,
/// but the entire test runs only in the secure world.
///
/// This just tests the secure-world interrupt handling
/// without world-switching.
fn test_secure_timer() -> TestResult {
    timer_test_main_loop(&secure_timer_helper)
}
