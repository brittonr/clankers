## Context

Current release/readiness gates prove plugin behavior in two different ways: Rust tests exercise Extism and stdio plugin logic directly, and Nix checks build/freshness-check shipped WASM plugin artifacts. The missing seam is a booted NixOS VM that uses the same installed package shape operators would run. Existing VM checks cover daemon smoke, remote daemon, session recovery, and NixOS module integration, but none intentionally prove plugin discovery or invocation from inside the VM.

## Goals / Non-Goals

**Goals:**
- Add a deterministic, credential-free NixOS VM check for packaged plugin runtime behavior.
- Cover one shipped safe Extism plugin and the reference stdio plugin fixture end-to-end.
- Make the check runnable as a named flake check and through `scripts/test-harness.sh vm vm-plugin-runtime`.
- Keep the test safe for CI and local readiness by avoiding live network credentials or external services.

**Non-Goals:**
- Do not contact GitHub, email, or other live plugin APIs.
- Do not expand plugin manifest/protocol semantics unless the VM reveals a packaging/runtime bug.
- Do not replace the existing Rust plugin unit/integration test suite.
- Do not require a graphical TUI session; command/daemon surfaces are enough for this VM check.

## Decisions

### Decision 1: Add one focused `vm-plugin-runtime` check

**Choice:** Add a new flake check named `vm-plugin-runtime` implemented under `nix/vm-tests/plugin-runtime.nix`.

**Rationale:** A separate check gives plugin runtime regressions a clear owner and keeps existing daemon/module VM tests from growing unrelated setup.

**Alternative:** Fold plugin assertions into `vm-smoke` or `vm-module-integration`. Rejected because plugin setup and fixture staging are distinct enough to deserve a named readiness rail.

**Implementation:** Import the check from `flake.nix`, add it to the VM harness `all` selector, and allow explicit selection with `./scripts/test-harness.sh vm vm-plugin-runtime`.

### Decision 2: Exercise safe local plugins only

**Choice:** Use deterministic local plugin tools: a safe shipped Extism tool such as hash/text-stats, plus `examples/plugins/clankers-stdio-echo` for stdio.

**Rationale:** The VM must be credential-free, network-independent, and stable in Nix sandbox or local KVM runs. Hash/text transformations and echo are deterministic and do not require secrets.

**Alternative:** Test GitHub/email plugins. Rejected because their meaningful paths depend on credentials and external APIs; existing tests already cover their no-token errors.

**Implementation:** Stage plugin roots into the VM, run the installed `clankers` binary or a small installed harness path that uses existing plugin commands/tool invocation, and assert on stable output strings/JSON receipts rather than internal files.

### Decision 3: VM proof should check packaging and runtime boundaries

**Choice:** Assert both plugin artifact discovery from packaged locations and runtime behavior after the VM boots.

**Rationale:** The gap is not just logic correctness; it is packaged path layout, dynamic runtime startup, and NixOS process environment behavior.

**Alternative:** Add another runCommand check. Rejected because runCommand cannot prove booted NixOS environment behavior, system PATH/environment differences, or process supervision semantics.

**Implementation:** The VM should verify installed plugin directories/manifests, invoke a safe Extism tool through clankers, install/stage the stdio fixture into a scanned plugin root, verify live tool registration/invocation, and confirm the stdio process exits cleanly on host shutdown or plugin disable.

## Risks / Trade-offs

**VM runtime cost** → Keep the scenario single-node and headless, and avoid full provider/model calls.

**CLI lacks a direct plugin invoke command** → Prefer existing user-facing surfaces if present; otherwise add the narrowest deterministic CLI/test harness seam that calls the same plugin host/tool adapter used by sessions.

**Sandbox differences across hosts** → Treat restricted sandbox assertions as Linux-only and fail-closed. If the VM cannot enable a restricted backend, assert that the plugin startup is refused with a clear error rather than silently skipping.

**Test brittleness from text output** → Prefer JSON/status receipts where available. If only text output exists, assert compact stable substrings and keep operator-facing docs aligned.

## Validation Plan

1. Evaluate the new check attribute with `nix eval .#checks.$system.vm-plugin-runtime.name`.
2. Run `nix build .#checks.$system.vm-plugin-runtime --no-link -L`.
3. Run `./scripts/test-harness.sh vm vm-plugin-runtime`.
4. Run focused plugin tests if implementation touches plugin runtime code.
5. Run `git diff --check` and update readiness docs if the VM check becomes part of the standard release gate.
