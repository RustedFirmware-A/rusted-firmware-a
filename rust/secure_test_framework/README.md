# Secure Test Framework

This directory contains the Secure Test Framework, a framework for running integration tests for
RF-A which may have both secure and non-secure components. It builds a binary to run in secure world
as BL32, and another binary which runs in normal world as BL33. These communicate over FF-A direct
messages to co-ordinate running tests.

Tests are currently divided into two main categories:

1. Secure tests, which run only in secure world. These are in the `secure_tests` module.
2. Normal-world tests, which are started from normal world but may also have a secure world
   component. These are in the `normal_world_tests` module.
