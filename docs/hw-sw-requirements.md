# Hardware and Software Requirements

This document defines the requirements the target system must meet to ensure correct RF-A behaviour.

## Hardware Requirements

The CPUs must implement the Armv9.0 extension or later. All CPU features made mandatory from the
Armv9.0 extension are assumed to be present ; RF-A might make use of them without querying their
support through id registers.

Also, the SoC must implement hardware-assisted power management. This is the case for all recent
Arm A-class CPUs, as they implement Arm DynamIQ Shared Unit (DSU). This greatly simplifies power
management code in RF-A, as no cache management operations are required during power down.

## Software Requirements

RF-A only supports v1.2 of the [Arm Firmware Framework for Arm A-profile (FF-A)][1], and will
support later versions in the future. Backwards-compatibility with FF-A v1.1 or earlier is not
provided. This means that all other firmware components interfacing with RF-A must support FF-A v1.2
at minimum as well.

This decision keeps the FF-A service code simple, especially in the areas of FF-A version
negotiation and calling convention. Given the current FF-A support landscape, this is not expected
to be a blocker for RF-A adoption.

RF-A only supports the Extended StateID format for the `power_state` parameter of PSCI `CPU_SUSPEND`
calls. This format was introduced long ago in PSCI version 1.0, i.e. back in 2015. This means that
RF-A won't work correctly with software that only supports the old, "original" format from PSCI
version 0.2.

[1]: https://developer.arm.com/documentation/den0077/latest
