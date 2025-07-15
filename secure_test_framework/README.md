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
