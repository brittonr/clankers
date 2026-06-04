## Why

Clankers plugin runtime coverage is strong at the Rust/nextest layer, and Nix already builds shipped WASM plugin artifacts, but there is no NixOS VM check that boots the packaged system and proves plugins work from the installed `clankers` surface. That leaves a release-readiness gap: plugin discovery, packaged artifact layout, stdio fixture execution, and daemon/TUI-facing command paths can regress in a NixOS environment even while unit tests pass.

## What Changes

- **Plugin VM check**: Add a flake-exported NixOS VM check that installs `clankersPkg` and packaged plugin artifacts, boots a VM, and exercises real plugin discovery/tool paths.
- **Extism runtime evidence**: Invoke at least one safe shipped Extism plugin tool, such as hash or text-stats, through the installed clankers/plugin tool surface.
- **Stdio runtime evidence**: Install or stage the reference stdio echo fixture and verify launch, ready handshake, live tool registration, invocation, and shutdown from the packaged host.
- **Harness integration**: Wire the check into the VM harness selector so plugin VM coverage is easy to run and appears in readiness summaries.

## Capabilities

### New Capabilities
- `plugin-nixos-vm-tests`: VM-backed proof that plugin discovery and safe plugin tool invocation work in a booted NixOS environment.

### Modified Capabilities
- `wasm-plugin`: Adds packaged NixOS runtime evidence beyond artifact freshness and Rust integration tests.
- `process-extension-runtime`: Adds packaged NixOS runtime evidence for stdio plugin lifecycle behavior.
- `process-extension-sandboxing`: Adds VM-level verification for restricted-mode availability/fail-closed behavior where practical.

## Impact

- **Files**: Expected changes under `nix/vm-tests/`, `flake.nix`, `scripts/test-harness.sh`, and possibly docs/readiness notes.
- **APIs**: No public protocol changes required; the VM should use existing CLI/daemon/plugin surfaces.
- **Dependencies**: No new runtime dependency is expected beyond NixOS test inputs already used by other VM checks.
- **Testing**: Verify with `nix build .#checks.$system.vm-plugin-runtime --no-link -L`, `./scripts/test-harness.sh vm vm-plugin-runtime`, focused plugin nextest checks, and `git diff --check`.
