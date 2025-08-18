# Contributing to RF-A

RF-A is an open source project, but is currently still in an early stage of development. At this
stage contributions from Arm and Google will be prioritised. This will change once it reaches
maturity and is ready for production usage.

Contributions should follow the [style guide](style-guide.md).

## Automated checks

Make sure to run `cargo fmt` on all Rust code before uploading a change.

`make clippy-test`, `make PLAT=fvp clippy` and `make PLAT=qemu clippy` should not produce any errors
or warnings.

Run `tools/pre-push` to run unit tests and build for a standard set of configurations and platforms.

## Review policy

To be merged, a change must have at least four votes in Gerrit:

1. Verified +1: This should be set by the CI bot automatically when CI passes.
2. Unsafe-Review +1: This may be set by CI automatically if your change doesn't touch any unsafe
   code. Otherwise, it must be given by a designated [unsafe reviewer][unsafe-reviewers]. Unsafe
   reviewers are allowed to give Unsafe-Review +1 to their own changes.
3. Code-Review +1: This may be given by any contributor other than the author of the change or the
   reviewer who gives +2.
4. Code-Review +2: This can only be given by a [maintainer][maintainers]. A maintainer must not
   +2 their own change.

[unsafe-reviewers]: https://review.trustedfirmware.org/admin/groups/0438a39457c1a5c0e2e648bd5d51a57e7da9303f,members
[maintainers]: https://review.trustedfirmware.org/admin/groups/2b67a42919bb2c91a8e6e41d1486ccfb2cac6697,members
