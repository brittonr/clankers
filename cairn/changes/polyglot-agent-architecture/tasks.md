# Tasks: Polyglot Agent Architecture

## Architecture contracts

- [ ] [serial] A1: Add a top-level architecture note or crate-level documentation that records the Nickel/Rust/Steel/Wasm/UCAN division of labor and explicitly distinguishes Steel trusted orchestration from Wasm untrusted tool execution [r[polyglot-agent-architecture.division-of-labor]]
- [ ] [parallel] A2: Add an architecture checker that rejects direct Steel interpreter, live Nickel evaluation, or Wasm runtime dependencies from generic engine/core/schema crates outside approved adapter modules [r[polyglot-agent-architecture.verification-rails.dependency-boundary]]
- [ ] [serial] A3: Update user-facing docs/status wording to avoid Steel and Wasm sandbox overclaims while still documenting the host-enforced capability model [r[polyglot-agent-architecture.steel-orchestration.no-sandbox-overclaim]] [r[polyglot-agent-architecture.wasm-tool-sandbox.no-magic-sandbox-claim]]

## Nickel agent contracts

- [x] [serial] N1: Define a Nickel-authored agent profile contract for persona metadata, prompt templates, model/profile fields, runtime budgets, tool manifests, JSON schemas, and compatibility metadata; current slice adds `policy/polyglot-agent/agent-profile.ncl` and exported JSON [r[polyglot-agent-architecture.nickel-agent-contracts]]
- [x] [parallel] N2: Add positive and negative exported fixtures for valid profile activation, missing prompt variables, malformed tool schemas, unsupported model/profile fields, and duplicate names; current slice includes a valid exported profile plus `invalid-agent-profile.json` for prompt/default/model/tool/receipt failures [r[polyglot-agent-architecture.nickel-agent-contracts.prompt-template-validation]]
- [x] [serial] N3: Add Rust DTO loading/parity checks that compare exported tool manifests against registered host/plugin/disabled-placeholder tool registrations before activation; current `check-polyglot-agent-profile.rs` validates required Steel/Wasm tool schema shape, modes, abilities, defaults, budgets, prompt variables, and receipt redaction [r[polyglot-agent-architecture.nickel-agent-contracts.tool-schema-host-parity]]

## Rust authority and UCAN

- [ ] [serial] R1: Add a typed dynamic-runtime action envelope for Steel and Wasm requests that records requested host function/tool, target resource, profile, and receipt destination before side effects [r[polyglot-agent-architecture.rust-authority.typed-host-function-seam]]
- [ ] [parallel] R2: Route dynamic-runtime action authorization through Rust-owned policy, UCAN, disabled-tool/session capability, and profile checks before any host effect [r[polyglot-agent-architecture.rust-authority]] [r[polyglot-agent-architecture.ucan-runtime-authority]]
- [ ] [parallel] R3: Add safe deterministic receipts for allowed, policy-denied, UCAN-denied, disabled, and failed dynamic-runtime actions without raw prompts, credentials, compact UCAN tokens, provider payloads, or oversized bodies [r[polyglot-agent-architecture.rust-authority.host-owned-receipts]]

## Steel orchestration

- [ ] [serial] S1: Add a Steel orchestration profile that can run a deterministic reasoning/routing loop through typed fake host functions without gaining ambient authority [r[polyglot-agent-architecture.steel-orchestration]]
- [ ] [parallel] S2: Add hot-reload boundary tests proving a script change can alter routing decisions but cannot add host functions, enlarge budgets, or gain new capabilities without a profile/policy/UCAN change [r[polyglot-agent-architecture.steel-orchestration.hot-reload-boundary]]
- [ ] [parallel] S3: Add negative Steel fixtures for raw filesystem, shell, git, network, provider, credential, daemon, TUI, and native-tool access outside typed host functions [r[polyglot-agent-architecture.verification-rails.dynamic-runtime-fixtures]]

## Wasm tool execution

- [ ] [serial] W1: Define a Wasm tool/generative-code execution profile with explicit imports, memory/fuel/time budgets, host-provided input DTOs, and structured output receipts [r[polyglot-agent-architecture.wasm-tool-sandbox]]
- [ ] [parallel] W2: Add a deterministic ephemeral generated-code fixture that runs in a bounded Wasm context with no ambient filesystem/network imports and emits a safe receipt [r[polyglot-agent-architecture.wasm-tool-sandbox.ephemeral-generated-code]]
- [ ] [parallel] W3: Add negative Wasm fixtures for missing imports, over-budget execution, malformed tool schema, and denied host capability [r[polyglot-agent-architecture.verification-rails.dynamic-runtime-fixtures]]

## Cross-layer verification

- [ ] [serial] V1: Add an end-to-end fixture where Nickel validates an agent profile, Steel chooses a typed action, Rust authorizes it with UCAN, and a Wasm tool executes with explicit imports [r[polyglot-agent-architecture.division-of-labor.steel-wasm-complementary]]
- [ ] [parallel] V2: Add denial fixtures proving Nickel-allowed-but-no-UCAN and UCAN-present-but-Nickel-denied actions both fail closed before side effects [r[polyglot-agent-architecture.ucan-runtime-authority.policy-not-enough]] [r[polyglot-agent-architecture.ucan-runtime-authority.ucan-not-enough]]
- [ ] [parallel] V3: Add receipt redaction checks for all cross-layer positive/negative fixtures [r[polyglot-agent-architecture.rust-authority.host-owned-receipts]]

## Final gates

- [ ] [serial] G1: Run `nix run .#cairn -- validate --root .` [r[polyglot-agent-architecture.verification-rails]]
- [ ] [serial] G2: Run `nix run .#cairn -- gate proposal polyglot-agent-architecture --root .` [r[polyglot-agent-architecture.verification-rails]]
- [ ] [serial] G3: Run `nix run .#cairn -- gate design polyglot-agent-architecture --root .` [r[polyglot-agent-architecture.verification-rails]]
- [ ] [serial] G4: Run `nix run .#cairn -- gate tasks polyglot-agent-architecture --root .` [r[polyglot-agent-architecture.verification-rails]]
- [ ] [serial] G5: Run `git diff --check` [r[polyglot-agent-architecture.verification-rails]]
