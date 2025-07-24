// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Framework for registering and running tests and their helpers.

mod expect;
pub mod protocol;

use crate::call_test_helper;
use alloc::boxed::Box;
use arm_ffa::Interface;
use linkme::distributed_slice;
use log::{debug, error, info};
use spin::Lazy;

/// The normal world tests.
#[distributed_slice]
pub static NORMAL_WORLD_TESTS: [NormalWorldTest];

/// The secure world tests.
#[distributed_slice]
pub static SECURE_WORLD_TESTS: [SecureWorldTest];

static NORMAL_WORLD_TESTS_SORTED: Lazy<Box<[&'static NormalWorldTest]>> = Lazy::new(|| {
    let mut tests = NORMAL_WORLD_TESTS.iter().collect::<Box<[_]>>();
    tests.sort();
    tests
});

static SECURE_WORLD_TESTS_SORTED: Lazy<Box<[&'static SecureWorldTest]>> = Lazy::new(|| {
    let mut tests = SECURE_WORLD_TESTS.iter().collect::<Box<[_]>>();
    tests.sort();
    tests
});

/// Returns an iterator over all normal world tests, sorted by name, along with their indices.
pub fn normal_world_tests() -> impl Iterator<Item = (usize, &'static NormalWorldTest)> {
    NORMAL_WORLD_TESTS_SORTED.iter().copied().enumerate()
}

/// Returns an iterator over all secure world tests, sorted by name, along with their indices.
pub fn secure_world_tests() -> impl Iterator<Item = (usize, &'static SecureWorldTest)> {
    SECURE_WORLD_TESTS_SORTED.iter().copied().enumerate()
}

/// Returns the number of normal world tests.
pub fn normal_world_test_count() -> usize {
    NORMAL_WORLD_TESTS.len()
}

/// Returns the number of secure world tests.
pub fn secure_world_test_count() -> usize {
    SECURE_WORLD_TESTS.len()
}

/// A single normal-world test.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct NormalWorldTest {
    pub name: &'static str,
    pub functions: TestFunctions,
    /// A secure-world handler for FF-A interfaces. This can return `None` if it doesn't want to
    /// handle the interface.
    pub secure_handler: Option<fn(Interface) -> Option<Interface>>,
}

impl NormalWorldTest {
    /// Returns the name of the test, including the module path.
    pub fn name(&self) -> &'static str {
        // Remove the crate name, if there is one.
        match self.name.split_once("::") {
            Some((_, rest)) => rest,
            None => &self.name,
        }
    }
}

#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub enum TestFunctions {
    NormalWorldOnly {
        function: fn() -> Result<(), ()>,
    },
    NormalWorldWithHelper {
        function: fn(&TestHelperProxy) -> Result<(), ()>,
        helper: fn([u64; 3]) -> Result<[u64; 4], ()>,
    },
}

/// A single secure-world test.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct SecureWorldTest {
    pub name: &'static str,
    pub function: fn() -> Result<(), ()>,
}

impl SecureWorldTest {
    /// Returns the name of the test, including the module path.
    pub fn name(&self) -> &'static str {
        // Remove the crate name, if there is one.
        match self.name.split_once("::") {
            Some((_, rest)) => rest,
            None => &self.name,
        }
    }
}

pub type TestHelperRequest = [u64; 3];
pub type TestHelperResponse = [u64; 4];

/// A proxy to call the secure-world helper function for a normal-world test.
pub type TestHelperProxy = dyn Fn(TestHelperRequest) -> Result<TestHelperResponse, ()>;

/// Runs the normal world test with the given index.
///
/// This should only be called from the normal world (BL33) part of STF.
#[allow(unused)]
pub fn run_normal_world_test(test_index: usize, test: &NormalWorldTest) -> Result<(), ()> {
    info!("Running normal world test {}: {}", test_index, test.name());
    match test.functions {
        TestFunctions::NormalWorldOnly { function } => function(),
        TestFunctions::NormalWorldWithHelper { function, .. } => {
            function(&move |args| call_test_helper(test_index, args))
        }
    }
}

/// Runs the secure world test with the given index.
///
/// This should only be called from the secure world (BL32) part of STF.
#[allow(unused)]
pub fn run_secure_world_test(test_index: usize) -> Result<(), ()> {
    if let Some(test) = SECURE_WORLD_TESTS_SORTED.get(test_index) {
        debug!("Running secure world test {}: {}", test_index, test.name());
        (test.function)()
    } else {
        error!("Requested to run unknown test {}", test_index);
        Err(())
    }
}

/// Runs the secure world test helper for the normal world test with the given index.
///
/// This should only be called from the secure world (BL32) part of STF.
#[allow(unused)]
pub fn run_test_helper(test_index: usize, args: [u64; 3]) -> Result<[u64; 4], ()> {
    debug!("Running secure world test helper {}", test_index);
    if let Some(test) = NORMAL_WORLD_TESTS_SORTED.get(test_index) {
        if let TestFunctions::NormalWorldWithHelper { helper, .. } = test.functions {
            helper(args)
        } else {
            error!("Requested to run helper for test without one.");
            Err(())
        }
    } else {
        error!("Requested to run unknown test helper {}.", test_index);
        Err(())
    }
}

/// Calls the secure world FF-A handler for the normal world test with the given index.
///
/// Returns `None` if there is no handler for the given test index.
///
/// This should only be called from the secure world (BL32) part of STF.
#[allow(unused)]
pub fn run_test_ffa_handler(test_index: usize, interface: Interface) -> Option<Interface> {
    let handler = NORMAL_WORLD_TESTS_SORTED.get(test_index)?.secure_handler?;
    debug!("Running test {} FF-A handler", test_index);
    handler(interface)
}

/// Registers a normal world test with the test framework.
#[macro_export]
macro_rules! normal_world_test {
    ($function:ident) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::framework::NORMAL_WORLD_TESTS)]
            static [<_NORMAL_WORLD_TEST_ $function:upper>]: $crate::framework::NormalWorldTest = $crate::framework::NormalWorldTest {
                name: concat!(module_path!(), "::", ::core::stringify!($function)),
                functions: $crate::framework::TestFunctions::NormalWorldOnly { function: $function },
                secure_handler: None,
            };
        }
    };
    ($function:ident, helper = $helper:ident) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::framework::NORMAL_WORLD_TESTS)]
            static [<_NORMAL_WORLD_TEST_ $function:upper>]: $crate::framework::NormalWorldTest = $crate::framework::NormalWorldTest {
                name: concat!(module_path!(), "::", ::core::stringify!($function)),
                functions: $crate::framework::TestFunctions::NormalWorldWithHelper {
                    function: $function,
                    helper: $helper,
                },
                secure_handler: None,
            };
        }
    };
    ($function:ident, handler = $handler:ident) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::framework::NORMAL_WORLD_TESTS)]
            static [<_NORMAL_WORLD_TEST_ $function:upper>]: $crate::framework::NormalWorldTest = $crate::framework::NormalWorldTest {
                name: concat!(module_path!(), "::", ::core::stringify!($function)),
                functions: $crate::framework::TestFunctions::NormalWorldOnly { function: $function },
                secure_handler: Some($handler),
            };
        }
    };
    ($function:ident, helper = $helper:ident, handler = $handler:ident) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::framework::NORMAL_WORLD_TESTS)]
            static [<_NORMAL_WORLD_TEST_ $function:upper>]: $crate::framework::NormalWorldTest = $crate::framework::NormalWorldTest {
                name: concat!(module_path!(), "::", ::core::stringify!($function)),
                functions: $crate::framework::TestFunctions::NormalWorldWithHelper {
                    function: $function,
                    helper: $helper,
                },
                secure_handler: Some($handler),
            };
        }
    };
}

/// Registers a secure world test with the test framework.
#[macro_export]
macro_rules! secure_world_test {
    ($function:ident) => {
        paste::paste! {
            #[linkme::distributed_slice($crate::framework::SECURE_WORLD_TESTS)]
            static [<_SECURE_WORLD_TEST_ $function:upper>]: $crate::framework::SecureWorldTest = $crate::framework::SecureWorldTest {
                name: concat!(module_path!(), "::", ::core::stringify!($function)),
                function: $function,
            };
        }
    };
}
