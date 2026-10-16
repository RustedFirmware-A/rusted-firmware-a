# Pointer Authentication - FEAT_PAUTH

RF-A can leverage Pointer Authentication to protect against Return-Oriented Programming (ROP) attacks.

It is disabled by default and can be turned on by building RF-A with the `PAUTH_EL3=1` makefile
variable, which will request the compiler to instrument the code with PAC instructions and enable
the `pauth` feature flag.

## Activation

RF-A enables Pointer Authentication early in boot, after platform initialization, in order to
maximize the amount of code that runs under its protection. Later on, once the system is up and
running, and that the CPU re-enters EL3, RF-A first saves all Pointer Authentication registers that
might be used by the incoming lower EL, then it restores its own key with the value it had last time
execution was in EL3.

## Key management

RF-A only uses key A for authentication of instruction pointer values (APIAKey_EL1).

How often the keys are refreshed is a platform decision, as it is a tradeoff between security and
execution speed. On one hand, refreshing the keys more often might help to protect against some
classes of attacks (e.g. replay attacks) ; on the other hand, generating a new key from a reliable
source of entropy can be an expensive operation.

To accomodate with these trade-off decisions, key management in RF-A has some degree of
flexibility. It can range from one key per CPU per boot, to one single key for all CPUs for the
lifetime of the system.

More specifically, RF-A statically allocates storage for one key per CPU through
`PAUTH_APIAKEY_EL3`, and it calls into the platform through `Platform::init_apkey()` to retrieve the
key material each time a CPU powers on. The platform is then free to generate a fresh key everytime,
tweak a previous key value, or just return the same key every time, for example.

---

_Copyright The Rusted Firmware-A Contributors._
