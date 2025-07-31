# RF-A Threat Model

TF-A has 2 distinct threat models: a threat model for the firmware when running in a device, and a
supply chain threat model. For the most part, RF-A includes them by reference in their entirety.
However, the Rust language and ecosystem allow us to make some slightly stronger statements about
defenses.

## Firmware Threat Model

[The firmware threat
model](https://trustedfirmware-a.readthedocs.io/en/latest/threat_model/firmware_threat_model/threat_model.html)
covers a variety of relevant attacks and defenses from a variety of defender perspectives. Some of
them are independent of the differences between C and Rust; and for others, defense becomes easier
with a Rust implementation.

### Memory Safety

A key difference between C TF-A and RF-A is in how we can address threat ID 08, memory unsafety:

> Memory corruption due to memory overflows and lack of boundary checking when accessing resources
> could allow an attacker to execute arbitrary code, modify some state variable to change the normal
> flow of the program, or leak sensitive information

(Note that the notion of memory unsafety covers more than just memory **corruption** (i.e. writes);
it includes overreads and information leaks as well.)

Since most of RF-A is implemented in safe Rust, we have static and dynamic guarantees that attacks
employing memory unsafety bugs will be more rare and potentially more difficult. Thus, the big
picture change RF-A makes is that memory unsafety attacks should become substantially less likely,
and the threat analysis can be updated accordingly.

There is still some `unsafe` Rust and some assembly code, so the risk is not completely eliminated.
That said, we have much less assembly code in RF-A relative to C TF-A, and use safe and `unsafe`
Rust wherever possible.

We (intend to) mitigate the residual risk with common exploit mitigations such as [stack
canaries](https://github.com/RustedFirmware-A/rusted-firmware-a/issues/43), turning on [Rust’s
integer overflow checking even in release
builds](https://review.trustedfirmware.org/c/TF-A/trusted-firmware-a/+/37714), and [control-flow
integrity (CFI)](https://github.com/RustedFirmware-A/rusted-firmware-a/issues/45). We intend for
these mitigations to be on in the standard production configurations of RF-A.

### Type Safety And Logical Errors

To address threat ID 07,

> An attacker can perform a denial-of-service attack by using a broken SMC call that causes the
> system to reboot or enter into unknown state.

, we have some defenses against this in the forms of:

* easier and thus more frequent unit testing
* integration testing with TFTF (to come)
* use of the type system to shrink the space of possible machine states (e.g. `MutexGuard` to
  improve our ability to release locks at the right time and in the right order)

The RF-A Secure Test Framework (STF) also gives us test coverage for interactions between the Secure
and Non-secure worlds.

## Supply Chain Threat Model

[The TF-A supply chain threat
model](https://trustedfirmware-a.readthedocs.io/en/latest/threat_model/supply_chain_threat_model.html)
considers a variety of attacks against and defenses for both shared infrastructure (GitHub, Arm’s
Gerrit and Jenkins) and downstream development environments. Most of it is directly applicable to
RF-A, too.

A key difference between C TF-A and RF-A is that RF-A depends on a few third-party (3P) Rust crates,
(obviously) instead of C libraries like C TF-A. Other aspects of the supply chain security story,
such as continuous integration and the mail server, are the same or similar.

To address TFA-SC-DEP-02,

> An attacker can inject malicious code into TF-A external dependencies.

we use several techniques:

* minimizing the number of 3P crates
* using `cargo vet` (including for first-party crates) to ensure that they meet a reasonable safety
  bar (in paticular, `unsafe` blocks)
* Cargo’s version pinning

RF-A does not have the internal (i.e. vendored) vs. external dependency distinction that C TF-A
does; all crates are external.

To address TFA-SC-TOOL-01,

> Malicious code can be injected at build time through malicious tools.

, we can at least (and do) specify a specific Rust toolchain version in our rust-toolchain.toml
file. Although currently we build some assembly code files with the `cc` crate, when [Audit assembly
and header files copied from C TF-A](https://github.com/RustedFirmware-A/rusted-firmware-a/issues/7)
is complete, the toolchain will be somewhat less open-ended in that we will use only `rustc` and not
a full C front-end as well. (That will also eliminate our largest 3P dependency.)

--------------

*Copyright The Rusted Firmware-A Contributors*
