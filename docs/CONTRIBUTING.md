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

## Starting the CI bot

To start the CI bot on a change, an [approved developer][approved-developers] needs to set the
Allow-CI vote in Gerrit. The bot will then report its progress and results on Gerrit:

```
ci-bot
	"Build Started https://ci.trustedfirmware.org/job/rf-a-gerrit-pipeline/7/ "
```
```
ci-bot						Verified +1
	"Build Successful
	https://ci.trustedfirmware.org/job/rf-a-gerrit-pipeline/7/ : SUCCESS"
```

In case of failure, follow the link to the CI job to understand which test(s) failed and why. If the
information is not visible directly from the top-level page, check out the "Pipeline Console" page
(see link on the left-hand side). In some cases, exploring the log files in the "Build Artifacts"
can also be useful.

Note that Jenkins CI runs are only retained for a limited period of time. It may happen that a CI
job runs against your patch on a certain day, but as you only come back to it a few days later, by
that time it is gone. Hence you cannot look at the details of the CI failure. In that case, please
feel free to ping an [approved developer][approved-developers] to start a fresh CI run.

### If you start the CI bot yourself...

If you are part of the [approved developers][approved-developers], you may set the `Allow-CI` vote
by yourself on your own patches. In that case, the following details are important to know.

The `Allow-CI` trigger vote has 2 possible values:
- `Allow-CI+1` triggers lightweight tests only.
- `Allow-CI+2` triggers the full tests.

As a rule of thumb, and in an effort to make the best use of CI resources without affecting the
quality of testing:
- For a single patch, always use `Allow-CI+2`.
- For a patch stack, use `Allow-CI+2` on the top patch only, and `Allow-CI+1` on patches below.

Note that, at the end of the day, it is a judgement call on the developer / maintainer whether the
full tests need to run on a patch or not. As such, this policy is not enforced by Gerrit, and both
`Allow-CI+1` and `Allow-CI+2` will satisfy the submission requirements (assuming all selected tests
pass).

If for some reason, you need to re-trigger the CI on a patch, or you need to run the full set of
tests on a lightly-tested patch, you can set the `Allow-CI` vote to 0 then set it again to the
desired value.

[unsafe-reviewers]: https://review.trustedfirmware.org/admin/groups/0438a39457c1a5c0e2e648bd5d51a57e7da9303f,members
[maintainers]: https://review.trustedfirmware.org/admin/groups/2b67a42919bb2c91a8e6e41d1486ccfb2cac6697,members
[approved-developers]: https://review.trustedfirmware.org/admin/groups/12cd7f45d37c370b4de75bd8e2b736330b063b34,members
