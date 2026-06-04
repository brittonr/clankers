# Change: Harden Embedded SDK API Inventory

## Problem

`scripts/check-embedded-sdk-api.rs` currently scans simple top-level `pub` declarations with string parsing. It misses public methods, public fields, reexports, feature-gated items, and nested API exposure. That leaves the generated SDK inventory useful for broad drift detection but too weak as a compatibility contract for embedders.

## Goals

- Replace or augment string scanning with a typed Rust API inventory for green SDK crates.
- Track public types, methods, fields, modules, constants, functions, traits, type aliases, and root reexports.
- Distinguish stable, optional, experimental, compatibility, and unsupported/internal items with owner diagnostics.
- Keep the inventory deterministic and compatible with release-receipt hashing.

## Non-goals

- Do not require rustdoc JSON from an unstable toolchain unless the rail has a reliable fallback.
- Do not classify every desktop/root crate public item; focus on embedded SDK boundary owners.
- Do not turn inventory hardening into a semantic API stabilization decision for every experimental item.

## Proposed scope

Build a typed inventory rail using `syn`/Cargo metadata or another deterministic source parser, update generated inventory and stability policy to include methods/fields/reexports, and add self-tests that prove source-preserving refactors do not cause false failures.

## Verification

Focused validation should include the inventory rail, brick inventory stability rail, embedded SDK acceptance, a deliberate fixture/self-test for missed public methods and reexports, Cairn gates, and `git diff --check`.
