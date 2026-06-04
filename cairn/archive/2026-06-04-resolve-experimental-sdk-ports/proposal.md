# Change: Resolve Experimental SDK Ports

## Problem

The generated embedded SDK inventory currently lists dozens of `experimental` public items, especially `clankers-engine-host` observation ports and `clankers-tool-host` service/context APIs. Some are useful product seams, while others are unused or premature. Leaving a large experimental surface makes the SDK harder to explain and increases accidental compatibility expectations.

## Goals

- Classify each experimental port as promote-to-supported, keep-experimental-with-evidence, or make-private.
- Dogfood representative tool-host service/context APIs through executable fixtures before promotion.
- Remove or hide unused engine-host ports that are not wired by runner/examples.
- Update docs, inventory, and brick stability policy to reflect the reduced experimental budget.

## Non-goals

- Do not stabilize every tool service in one slice.
- Do not remove necessary app-edge adapter seams used by desktop Clankers without migration notes.
- Do not promote plugin supervision or built-in tool bundles into green SDK crates.

## Proposed scope

Start with an inventory and budget rail for experimental SDK ports, then resolve the first batch: unused engine-host ports and representative tool-host context/service APIs that can be proven by deterministic examples.

## Verification

Focused validation should include API inventory/stability rails, neutral tool context fixtures, embedded tool-kit/product-workbench examples, engine-host feature matrix where affected, `scripts/check-embedded-agent-sdk.rs`, Cairn gates, and `git diff --check`.
