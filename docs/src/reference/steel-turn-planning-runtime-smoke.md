# Steel Turn Planning Runtime Smoke

The Steel turn-planning runtime smoke proves the reviewed `steel.host.plan_turn` configuration path reaches a real controller prompt command and becomes visible to daemon/attach clients without granting Steel ambient authority.

## What the smoke covers

- A `SessionCommand::Prompt` runs through `SessionController`, `Agent`, and the normal turn loop.
- Missing `steelTurnPlanning` settings load the bundled reviewed `steel.host.plan_turn` profile/script by default.
- Explicit `steelTurnPlanning.enabled = false` keeps Rust-native planning and emits no Steel receipt.
- Explicit `steelTurnPlanning` profile/script settings still load a reviewed profile and script through the same activation helper.
- The script is checked by BLAKE3 before activation.
- The profile requires explicit session capability and UCAN ability strings.
- The Steel Scheme planner emits only a typed plan; authorized default receipts select the Steel execution adapter, which must evaluate `(host "steel.host.execute_turn")` and then Rust dynamic-runtime authority before provider/tool effects.
- The resulting `steel.host.plan_turn` receipt is bridged from `AgentEvent::SystemMessage` to `DaemonEvent::SystemMessage`, making planner/executor selection visible to daemon/attach clients.
- Default-settings smoke asserts the daemon-visible planning receipt includes `executor=SteelScheme` and also observes a `steel.host.execute_turn` execution receipt from the Steel-selected adapter with `host_call_status=Succeeded`, `host_call_reason=Ok`, `host_call_payload=Valid`, `authority_status=Allowed`, `authority_reason=Ready`, `required_ucan=clankers/steel/orchestrate.execute_turn`, and redacted host-call/authority receipt hashes.
- Comparison smoke asserts the planning receipt includes `executor=RustNative` and emits no Steel-selected execution receipt.
- Execution-authority smoke removes `turn-execution` / `clankers/steel/orchestrate.execute_turn`, observes a daemon-visible denied `steel.host.execute_turn` receipt with `host_call_reason=MissingHostCapability`, and proves the provider was not called before any provider request.
- Receipt text is redacted: no raw prompt, script body, credential, UCAN proof, provider payload, or tool body is exposed.

## Fail-closed checks

The smoke also covers invalid runtime activation:

- script hash mismatch fails before any provider call,
- missing planning session/UCAN authority fails before any Steel planning success receipt,
- missing execution host-call/session/UCAN authority fails before any provider call while emitting a redacted execution-host-call and authority denial receipt,
- invalid activation does not emit a success receipt.

This preserves the seam:

```text
Nickel = reviewed declaration/profile
UCAN   = runtime delegated authority strings
Rust   = validation, execution authority checks, host-effect execution, provider calls, daemon events, receipts
Steel  = typed planning plus explicit execute_turn host-call request, without ambient host authority
Wasm   = separate untrusted execution boundary
```

## Commands

Use the same linker override as other root integration tests on this machine to avoid the local mold bug:

```bash
CC=gcc CXX=g++ \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc \
  RUSTFLAGS='-C link-arg=-fuse-ld=bfd' \
  RUSTC_WRAPPER= \
  CARGO_TARGET_DIR=target/steel-runtime-smoke-test \
  cargo test -p clankers steel_runtime_smoke --test embedded_controller
```

Run the static checker:

```bash
./scripts/check-steel-turn-planning-runtime-smoke.rs
```

The checker writes its receipt to:

```text
target/steel-turn-planning-runtime-smoke/receipt.json
```
