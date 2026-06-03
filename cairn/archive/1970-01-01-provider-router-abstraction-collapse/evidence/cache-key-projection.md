Evidence-ID: provider-router-abstraction-collapse-cache-key-projection
Task-ID: V1,V2
Artifact-Type: command-log
Covers: provider-router-abstraction-collapse.duplicate-inventory, provider-router-abstraction-collapse.single-owner, provider-router-abstraction-collapse.thin-adapter, provider-router-abstraction-collapse.verification
Status: complete

# Provider Router Cache-Key Projection Collapse Evidence

## Implementation summary

- Added `crates/clankers-provider/src/provider_router_responsibility.rs`, an explicit provider/router concern inventory naming policy owners and compatibility boundaries for request, stream, auth, discovery, routing, retry/fallback/cooldown, cost, and error concerns.
- Selected `CacheKeyRequestProjection` as the collapsed concern.
- Added `router_request_bridge::compute_router_cache_key_from_request_projection(...)` so cache-key material uses the same router request JSON projection as routed backends.
- Changed `RouterProvider::compute_cache_key(...)` to delegate request/message shape to `router_request_bridge` instead of serializing `AgentMessage` internals inline.
- Added a literal cache-key fixture that compares against explicit provider-native router message JSON.
- Updated lego architecture rails and baseline to name the concern inventory and reject duplicate cache-key message projection in `router.rs`.

## Focused provider/router fixtures

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider router_request_bridge
```

Result: 3 tests run, 3 passed, 177 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider provider_router_responsibility
```

Result: 2 tests run, 2 passed, 178 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-provider request
```

Result: 13 tests run, 13 passed, 167 skipped.

## Cargo checks and architecture rails

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-provider --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clanker-router --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0; inventory written to `target/lego-architecture/dependency-ownership-inventory.json`.

```text
nix run .#cairn -- gate proposal provider-router-abstraction-collapse --root .
nix run .#cairn -- gate design provider-router-abstraction-collapse --root .
nix run .#cairn -- gate tasks provider-router-abstraction-collapse --root .
```

Result: all three gates returned `valid: true` and `verdict: PASS`.

```text
nix run .#cairn -- validate --root .
```

Result before archive: `valid: true`; 3 active changes and 54 specs validated.

```text
nix run .#cairn -- archive provider-router-abstraction-collapse --root . --execute
nix run .#cairn -- validate --root .
```

Result after archive: archive returned `mutated: true`; validation returned `valid: true` with 2 active changes and 53 specs validated.

```text
git diff --check
```

Result: exit status 0.
