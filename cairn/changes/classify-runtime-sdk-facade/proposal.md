# Change: Classify Runtime SDK Facade

## Problem

`clankers-runtime` is documented as a host-facing embedding facade, but embedded SDK policy treats it as yellow/app-edge rather than a green generic SDK crate. Its public boundary guard is a small hardcoded name list and does not inventory the actual exported API. This ambiguity makes it unclear whether runtime facade additions are SDK-stable, yellow app-edge, or accidental leakage.

## Goals

- Decide and document whether `clankers-runtime` is yellow-only, partially green through a smaller facade, or split into green/yellow crates.
- Add a real public API and dependency rail for the runtime-facing surface.
- Keep provider/auth/plugin/process/prompt filesystem/desktop services behind explicit injected adapters and fail-closed defaults.
- Align SDK guide, lego policy, API inventory, and release receipt with the chosen classification.

## Non-goals

- Do not promote desktop provider discovery, auth stores, plugin supervision, process backends, prompt/skill filesystem discovery, or session database ownership into green SDK crates.
- Do not break existing runtime facade consumers without migration notes.
- Do not duplicate provider/router policy inside runtime adapters.

## Proposed scope

Treat the first slice as a classification and rail-hardening change: inventory runtime public exports and dependencies, choose a target split/classification, update docs/policy, and add fail-closed boundary rails before moving substantial APIs.

## Verification

Focused validation should include runtime extension service matrix, config/prompt/skill service rails, provider-router runtime service rails, runtime public API/dependency inventory, embedded SDK acceptance, Cairn gates, and `git diff --check`.
