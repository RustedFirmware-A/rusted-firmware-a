// Copyright (c) 2025, Google LLC. All rights reserved.
//
// SPDX-License-Identifier: BSD-3-Clause

//! Macros for checking expectations in tests without panicking.

/// Logs an error and returns `Err(())` if the given expression is false.
#[macro_export]
macro_rules! expect {
    ($expectation:expr) => {
        if !$expectation {
            log::error!(
                "expectation failed at {}:{}:{}: {}",
                file!(),
                line!(),
                column!(),
                stringify!($expectation),
            );
            return Err(());
        }
    };
}

/// Logs an error and returns `Err(())` if the given expressions are not equal.
#[macro_export]
macro_rules! expect_eq {
    ($left:expr, $right:expr) => {{
        let left = $left;
        let right = $right;
        if left != right {
            log::error!(
                "expectation failed at {}:{}:{}: `{} == {}`",
                file!(),
                line!(),
                column!(),
                stringify!($left),
                stringify!($right),
            );
            log::error!("  left: {:?}", left);
            log::error!(" right: {:?}", right);
            return Err(());
        }
    }};
}
