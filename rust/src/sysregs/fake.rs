// Copyright (c) 2024, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Fake implementations of system register getters and setters for unit tests.

use std::sync::Mutex;

/// Values of fake system registers.
pub static SYSREGS: Mutex<SystemRegisters> = Mutex::new(SystemRegisters::new());

/// A set of fake system registers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SystemRegisters {
    pub elr_el1: u64,
    pub elr_el2: u64,
    pub esr_el1: u64,
    pub esr_el2: u64,
    pub hcr_el2: u64,
    pub sctlr_el1: u64,
    pub spsr_el1: u64,
    pub spsr_el2: u64,
    pub vbar_el1: u64,
    pub vbar_el2: u64,
}

impl SystemRegisters {
    const fn new() -> Self {
        Self {
            elr_el1: 0,
            elr_el2: 0,
            esr_el1: 0,
            esr_el2: 0,
            hcr_el2: 0,
            sctlr_el1: 0,
            spsr_el1: 0,
            spsr_el2: 0,
            vbar_el1: 0,
            vbar_el2: 0,
        }
    }
}

/// Generates a public function named `$function_name` to read the fake system register `$sysreg`.
macro_rules! read_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name() -> u64 {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg
        }
    };
}

/// Generates a public function named `$function_name` to write to the fake system register
/// `$sysreg`.
macro_rules! write_sysreg {
    ($sysreg:ident, $function_name:ident) => {
        pub fn $function_name(value: u64) {
            crate::sysregs::fake::SYSREGS.lock().unwrap().$sysreg = value;
        }
    };
}
