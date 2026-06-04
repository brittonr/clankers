# Verification

## Drain Verification Matrix

| Rail | Command | Status | Scope rationale |
| --- | --- | --- | --- |
| format | `cargo fmt --check` | pass | Rust formatting for config/tool/test changes. |
| config unit | `CARGO_TARGET_DIR=target cargo test -p clankers-config external_memory -- --nocapture` | pass: 5 passed | External memory config validation, including HTTP endpoint/credential policy. |
| tool unit | `CARGO_TARGET_DIR=target cargo test --lib external_memory -- --nocapture` | pass: 9 passed | Local/HTTP provider behavior, deterministic fake HTTP backend, replay-safe metadata, publication policy. |
| integration | `CARGO_TARGET_DIR=target cargo test --test external_memory -- --nocapture` | pass: 3 passed | Shared tool surface publication and local provider integration. |
| compile | `CARGO_TARGET_DIR=target cargo check --tests` | pass | Workspace test-target compilation for affected crates. |
| OpenSpec | `openspec validate add-remote-external-memory-providers --strict` | pass | Active change package validates before archive. |
| whitespace | `git diff --check` | pass | No whitespace errors in pending diff. |

## Inventory

Touched seams for `add-remote-external-memory-providers`:

- `crates/clankers-config/src/settings.rs`: `ExternalMemorySettings` config validation; HTTP provider now requires endpoint and credential env rather than being categorically unsupported.
- `src/tools/external_memory.rs`: Specialty tool adapter; local provider remains unchanged, HTTP provider now sends bounded search requests with timeout/credential policy and replay-safe metadata.
- `tests/external_memory.rs`: integration surface for tool publication and local provider behavior.
- `README.md` and `docs/src/reference/config.md`: user/operator docs for disabled-by-default remote provider behavior and prompt-injection non-goal.

Safety policy:

- Remote external memory remains disabled by default.
- Missing or blank credential env fails before network contact.
- `maxResults` bounds outbound query limits and response display.
- Tool-result details intentionally omit raw query text, memory result text, headers, tokens, and credential env values.
- `injectIntoPrompt` stays policy state only; this slice does not auto-inject remote memory into prompt assembly.
