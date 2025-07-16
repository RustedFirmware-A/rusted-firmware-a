# Secure Test Framework

This directory contains the Secure Test Framework, a framework for running integration tests for
RF-A which may have both secure and non-secure components. It builds a binary to run in secure world
as BL32, and another binary which runs in normal world as BL33. These communicate over FF-A direct
messages to co-ordinate running tests.

Tests are currently divided into two main categories:

1. Secure tests, which run only in secure world. These are in the `secure_tests` module.
2. Normal-world tests, which are started from normal world but may also have a secure world
   component. These are in the `normal_world_tests` module.

## Adding tests

Tests are registered with the framework via macro. For a secure world only test:

```rust
secure_world_test!(test_foo);
fn test_foo() -> Result<(), ()> {
   expect_eq!(42, 66);
   Ok(())
}
```

For a normal world only test:

```rust
normal_world_test!(test_foo);
fn test_foo() -> Result<(), ()> {
   expect_eq!(42, 66);
   Ok(())
}
```

For a normal world test with a secure world helper component:

```rust
normal_world_test!(test_foo, helper = foo_helper);
fn test_foo(helper: &TestHelperProxy) -> Result<(), ()> {
   let result = helper([41, 22, 0]);
   expect_eq!(result[0], 42);

   let result = helper([41, 5, 0]);
   expect_eq!(result[0], 25);

Ok(())
}

fn foo_helper(args: [u64; 3]) -> Result<[u64; 4], ()> {
   expect_eq!(args[0], 41);
   Ok([args[1] + 20, 0, 0, 0])
}
```

In this case, the test starts with `test_foo` being run in the normal world BL33, but calls to
`helper` will result in `foo_helper` being run in the secure world BL32 with the given arguments.
This can be used to write tests where components in both worlds need to communicate.

### `FF-A`-specific test utils

For testing FF-A interfaces on the Secure Test Framework, there are specific convenience macros and
functions that are provided under `util.rs` that may be useful when implementing both secure and
normal world tests.

```rust
normal_world_test!(test_ffa_foo, handler = foo_handler);
fn test_ffa_foo() -> Result<(), ()> {

   // Make a FF-A SMC call with Interface::Foo, expecting that it will be forwarded to Secure World.
   // In case of an FF-A call error, log the message "FOO fail" and return an error.
   // Check that the interface that is returned from secure world is "Success"
   let args = expect_ffa_interface!(
      expect_ffa_success,
      "FOO failed",
      ffa::foo(foo_parameter)
   );

   // Check that the arguments of that "Success" interface are what we'd expected.
   expect_eq!(args, SuccessArgs::Args32([0, 0, 0, 0, 0, 0]));
   Ok(())
}

fn foo_handler(interface: Interface) -> Option<Interface> {
   // Interface the `foo_handler` is expecting.
   let Interface::Foo { foo_parameter } = interface else {
      return None;
   };

   // Check that the interface's parameters are the expected ones (forwarding to secure world has
   // happened correctly).
   assert_eq!(foo_parameter, 102);

   // Return the interface that the secure world is expected to respond to normal world with.
   // This will not always be the "Success" interface and will depend on the actual interface the
   // `foo_handler` is expecting.
   Some(Interface::Success {
      args: SuccessArgs::Args32([0, 0, 0, 0, 0, 0]),
      target_info: TargetInfo {
         endpoint_id: 0,
         vcpu_id: 0,
      },
   })
}
```

In this case, the test starts with `test_ffa_foo` being run in the normal world BL33 and triggering
an FF-A SMC call with a given FF-A interface. FF-A interfaces passed to the secure world BL32 which
the test framework doesn't otherwise handle will result in `foo_handler` being run in the secure
world.
The `handler` is registered through the `normal_world_test!(test_ffa_foo, handler = foo_handler)`
macro (as with the `helper` on the above section) and will be automatically called by the framework.
This can be used to write tests where components in both worlds need to communicate via FF-A and
check that the correct forwarding from normal world to secure world (and back) works correctly.

```rust
macro_rules! expect_ffa_interface {
    ($expect:ident, $message:expr, $call:expr) => {
        $expect(crate::util::log_error($message, $call)?)?
    };
}
```

As shown in `test_ffa_foo`, the `expect_ffa_interface!` macro may be used to:

1. Trigger the test's SMC call with the given interface via the `$call` function parameter
2. Log an error `$message` if an error has happened during SMC call invocation
3. Check that the interface returned by secure world is the expected one via the `$expect` function.
