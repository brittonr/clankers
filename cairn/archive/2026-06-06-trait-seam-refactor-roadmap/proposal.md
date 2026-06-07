# Change: Trait Seam Refactor Roadmap

## Why

Several remaining coupling hotspots now have two or more concrete implementations, but the code still carries enum matches, runtime-kind checks, or duplicated shell adapters. Traits should be introduced where they make ownership clearer and behavior easier to test, not as blanket abstraction.

## What Changes

- Add an explicit trait-seam requirement to the remaining coupling drain plan.
- Prioritize trait extraction for plugin runtimes, OAuth provider flows, framed session transports, session storage formats, and process-job shell services.
- Require each new trait seam to name the policy owner, adapter boundary, migration path, and focused verification rail.

## Impact

- **Files**: `crates/clankers-plugin`, `crates/clankers-provider`, daemon/attach transport modules, `crates/clankers-session`, `src/tools/process`, and architecture/source-boundary rails.
- **Testing**: mixed plugin runtime tests, provider auth/request tests, attach/remote transport tests, session storage tests, process-job backend tests, Cairn gates, and closeout validation.
