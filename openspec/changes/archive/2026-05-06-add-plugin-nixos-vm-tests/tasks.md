## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta specs for `add-plugin-nixos-vm-tests`.
- [x] Validate the OpenSpec package with `openspec validate add-plugin-nixos-vm-tests --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory existing VM checks, plugin package outputs, shipped plugin artifact paths, and stdio fixture locations.
- [x] Add a `vm-plugin-runtime` NixOS VM check that boots with the packaged `clankers` binary and packaged plugin artifacts.
- [x] Add deterministic Extism plugin assertions for discovery, load, and safe tool invocation.
- [x] Add deterministic stdio fixture assertions for launch, ready handshake, tool registration, invocation, and clean shutdown/disable.
- [x] Add restricted-sandbox VM assertions that prove enforcement when available and fail-closed behavior otherwise.
- [x] Wire the check into `flake.nix` and `scripts/test-harness.sh vm vm-plugin-runtime`, including summary reporting.
- [x] Update release-readiness/plugin docs to mention the new VM coverage and how to run it.

## Phase 3: Verification and Closeout

- [x] Run `nix build .#checks.$system.vm-plugin-runtime --no-link -L`.
- [x] Run `./scripts/test-harness.sh vm vm-plugin-runtime`.
- [x] Run focused plugin tests if runtime code changes are required.
- [x] Run `git diff --check`.
- [x] Sync the delta specs into canonical specs and archive the change after implementation tasks complete.

## Evidence

- `./scripts/test-harness.sh vm vm-plugin-runtime` passed on 2026-05-06.
- `cargo fmt --check` passed on 2026-05-06.
- `cargo check --tests` passed on 2026-05-06.
- `cargo nextest run -p clankers-plugin -p clanker-plugin-sdk -p clankers plugin --no-fail-fast` passed on 2026-05-06 with 303 passed, 1075 skipped.
- `nix eval --raw .#checks.$(nix eval --raw --impure --expr builtins.currentSystem).vm-plugin-runtime.name` returned `vm-test-run-clankers-vm-plugin-runtime`.
- `openspec validate add-plugin-nixos-vm-tests --strict` passed on 2026-05-06.
- `git diff --check` passed on 2026-05-06.
