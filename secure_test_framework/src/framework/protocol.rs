// Copyright The Rusted Firmware-A Contributors.
//
// SPDX-License-Identifier: BSD-3-Clause

//! The protocol used for the BL32 and BL33 parts of STF to communicate over FF-A direct messages.

use arm_ffa::DirectMsgArgs;
use thiserror::Error;

/// Value sent by a direct message to run a secure test.
const RUN_SECURE_TEST: u64 = 1;

/// Value sent by a direct message to call a test helper.
const RUN_TEST_HELPER: u64 = 2;

/// Value sent by direct message to register the start of a normal-world test.
const START_TEST: u64 = 3;

/// Value sent by direct message to register the end of a normal-world test.
const STOP_TEST: u64 = 4;

/// Value returned in a direct message response for a test success.
const TEST_SUCCESS: u64 = 0;

/// Value returned in a direct message response for a test failure.
const TEST_FAILURE: u64 = 1;

/// Value returned in a direct message response for a test panic. No further tests should be run
/// after this.
const TEST_PANIC: u64 = 2;

/// Requests sent from BL33 to BL32.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Request {
    /// Run a secure test.
    RunSecureTest { test_index: usize },
    /// Run the secure helper component of a normal-world test.
    RunTestHelper { test_index: usize, args: [u64; 3] },
    /// Register that the given normal world test is starting, so its FF-A handler should be used.
    StartTest { test_index: usize },
    /// Register that the current normal world test has finished, so its FF-A handler should no
    /// longer be used.
    StopTest,
}

impl From<Request> for DirectMsgArgs {
    fn from(request: Request) -> Self {
        DirectMsgArgs::Args64(match request {
            Request::RunSecureTest { test_index } => [RUN_SECURE_TEST, test_index as u64, 0, 0, 0],
            Request::RunTestHelper { test_index, args } => [
                RUN_TEST_HELPER,
                test_index as u64,
                args[0],
                args[1],
                args[2],
            ],
            Request::StartTest { test_index } => [START_TEST, test_index as u64, 0, 0, 0],
            Request::StopTest => [STOP_TEST, 0, 0, 0, 0],
        })
    }
}

impl TryFrom<DirectMsgArgs> for Request {
    type Error = ParseRequestError;

    fn try_from(args: DirectMsgArgs) -> Result<Self, ParseRequestError> {
        if let DirectMsgArgs::Args64(args) = args {
            match args[0] {
                RUN_SECURE_TEST => Ok(Self::RunSecureTest {
                    test_index: args[1] as usize,
                }),
                RUN_TEST_HELPER => Ok(Self::RunTestHelper {
                    test_index: args[1] as usize,
                    args: [args[2], args[3], args[4]],
                }),
                START_TEST => Ok(Self::StartTest {
                    test_index: args[1] as usize,
                }),
                STOP_TEST => Ok(Self::StopTest),
                request_code => Err(ParseRequestError::InvalidRequestCode(request_code)),
            }
        } else {
            Err(ParseRequestError::InvalidDirectMsgType(args))
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum ParseRequestError {
    #[error("Unexpected direct message request code {0}")]
    InvalidRequestCode(u64),
    #[error("Unexpected direct message request {0:?}")]
    InvalidDirectMsgType(DirectMsgArgs),
}

/// Responses sent from BL32 back to BL33.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Response {
    /// A secure test passed, or a secure helper returned successfully.
    Success { return_value: [u64; 4] },
    /// A secure test or secure helper failed.
    Failure,
    /// Something panicked in secure world.
    Panic,
}

impl From<Response> for DirectMsgArgs {
    fn from(response: Response) -> Self {
        DirectMsgArgs::Args64(match response {
            Response::Success { return_value } => [
                TEST_SUCCESS,
                return_value[0],
                return_value[1],
                return_value[2],
                return_value[3],
            ],
            Response::Failure => [TEST_FAILURE, 0, 0, 0, 0],
            Response::Panic => [TEST_PANIC, 0, 0, 0, 0],
        })
    }
}

impl TryFrom<DirectMsgArgs> for Response {
    type Error = ParseResponseError;

    fn try_from(args: DirectMsgArgs) -> Result<Self, ParseResponseError> {
        if let DirectMsgArgs::Args64(args) = args {
            match args[0] {
                TEST_SUCCESS => Ok(Self::Success {
                    return_value: [args[1], args[2], args[3], args[4]],
                }),
                TEST_FAILURE => Ok(Self::Failure),
                TEST_PANIC => Ok(Self::Panic),
                response_code => Err(ParseResponseError::InvalidResponseCode(response_code)),
            }
        } else {
            Err(ParseResponseError::InvalidDirectMsgType(args))
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum ParseResponseError {
    #[error("Unexpected direct message response code {0}")]
    InvalidResponseCode(u64),
    #[error("Unexpected direct message response {0:?}")]
    InvalidDirectMsgType(DirectMsgArgs),
}
