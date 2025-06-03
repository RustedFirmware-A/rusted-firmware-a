# RF-A Code Style Guide

Ideally, we will have automated means of checking as much of this guide as possible, and will
support these guidelines with helpful APIs, by using the type system, and so on. We will continually
reduce the manual effort necessary to adhere to these guidelines.

This document, and any supporting code, are subject to change. Additionally, there is likely to be
code and documentation that doesn’t meet these guidelines. If you see a way to get closer to the
ideal this document describes, would like to propose a policy change, or have an idea for an
automated check, please go right ahead and push a change to Gerrit.

## How To Use `unsafe`

As part of meeting [the RF-A threat model][1], we aim to have as little `unsafe` code as possible.
Keep `unsafe` as short as possible, and always write a safety comment explaining how the code
upholds Rust’s safety guarantees. (The `undocumented_unsafe_blocks` and `missing_safety_doc` Clippy
lints are set to “deny”.)

Safety comments should make specific and verifiable claims about the validity of pointers, the
block’s (non-)impingement on memory safety and type safety, and (where applicable) how the block
meets the `# Safety` requirements in the surrounding interface’s Rustdoc.

During code review, get an `unsafe` expert to review your `unsafe` blocks and their safety comments.
(Automatically requiring review is, of course, a key goal.)

## Third-party Dependencies

As part of meeting [the RF-A threat model][1], we aim to minimize third-party crate dependencies.
Only import a third-party crate if it’s absolutely necessary, and only use the minimum necessary
crate features.

For `[build-dependencies]`, convenience and clarity can be acceptable reasons to import a 3P crate
(for example, `anyhow`). Even then, parsimony remains a goal.

### Use `cargo vet`

Ultimately, we will require that `cargo vet` pass cleanly before submitting new dependencies. TODO:
Add a presubmit script to run `cargo vet`, and update this text once that is done.

The crate’s vetter and the crate’s author must not be the same person.

## Coding Guidelines

### Using The Primitive Integral Types

[`usize` and `isize`][2] are the Rust types for values that will become pointers, sequence indices,
and memory object counts.

`u32`, `i32`, `u64`, and `i64` are the right types for other register values, bit fields and bit
masks, and other integer values.

Inevitably, we will need to cast these types to each other and to pointers. We must do so carefully
to avoid unintended sign extension, truncation, erasure of pointer provenance, and unportability. A
bare cast from (for example) `usize` to `u64` using `as` risks creating those problems — while we
currently target 64-bit machines only, that is not guaranteed to always be true. The safest and most
future-proof approach for the integer types is to use `foo.try_into().unwrap()`, which will either
fail immediately upon use or (very likely) be optimized away.

TODO: Develop a standard, ergonomic way to safely convert between these types, and recommend them
here.

Converting between integer types and pointers is dangerous. In addition to the risks of (for
example) incorrect truncation or sign extension, such conversions also risk the elision of [pointer
provenance][3] information. Where applicable, prefer the [strict provenance][4] and [exposed
provenance][5] APIs instead of bare casts with (for example) `as`.

In the future, we might start using [the `lossy_provenance_casts` lint][6] to help us catch such
cases.

### `assert` And `debug_assert`

Use `assert` and `debug_assert` freely to check and enforce your assumptions. `assert`s run in debug
and release builds, while `debug_assert`s run only in debug builds.

In general, prefer `assert` unless it causes RF-A to miss some critical efficiency goal (latency,
object code size, or other). Consider `debug_assert` to be, in effect, an optimization: the
optimization is safe only if we are certain that the condition the assert tests can only fail due to
(statically determinable) programmer error. When the condition may be affected by run-time
conditions or by likely code churn — RF-A is a work in progress — `assert` is the way to go.

### `cargo clippy`

The RF-A project has the eventual goal of defining a set of clippy lints and then staying clean of
those lints, as a way of ensuring a baseline of code quality and readability.

Being mechanically generated, lints don’t always directly describe the problem; sometimes they only
detect a symptom (possibly even a 2nd-order symptom). For each lint, think carefully about the
ultimate cause and seek to solve that. Don’t quiet lints just for the sake of quieting them.

## Documentation

Document all `pub` interfaces (functions, types, methods, etc.) with Rustdoc comments (`/// ...` and
`//! ...`). (The `missing_docs` lint is set to “deny” in Cargo.toml.)

Use Markdown to indicate structure where necessary, especially for code identifiers but also for
emphasis, hyperlinks ([including links to related code][7]), and so on.

It’s always worth spending time on punctuation, capitalization (including proper nouns, Arm
architecture technical terms, and acronyms), and spelling. TODO: See if Arm has a documentation
style guide we can follow.

The primary language for comments and naming must be International English. In cases where there is
a conflict between the American English and British English spellings of a word, use the American
English spelling. However, for proper nouns, such as the names of companies, use the existing
spelling.

Rustdoc comments for functions and methods should begin with a summary fragment starting with a
verb. This should be phrased such that it could be preceded by "This function". For example:

```rust
/// Computes the sum of the given numbers.
fn sum(numbers: &[u32]) -> u32 {
    //...
}
```

### Copyrights

At the top of each source code file, put a copyright header in this format:

> Copyright The Rusted Firmware-A Contributors.
>
> SPDX-License-Identifier: BSD-3-Clause

[1]: threat-model.md
[2]: https://doc.rust-lang.org/reference/types/numeric.html#machine-dependent-integer-types
[3]: https://doc.rust-lang.org/stable/std/ptr/index.html#provenance
[4]: https://doc.rust-lang.org/stable/std/ptr/index.html#strict-provenance
[5]: https://doc.rust-lang.org/stable/std/ptr/index.html#exposed-provenance
[6]: https://doc.rust-lang.org/rustc/lints/listing/allowed-by-default.html#lossy-provenance-casts
[7]: https://doc.rust-lang.org/rustdoc/write-documentation/linking-to-items-by-name.html
