Artifact-Type: validation-log
Task-ID: I23,V22
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved simple skill-resolution DTOs to neutral message contracts:

- Added `clanker_message::SkillResolutionRequest` for host skill-service lookup requests.
- Added `clanker_message::ResolvedSkillSnippet` for host-resolved skill snippets.
- Re-exported those DTOs through `clankers-runtime::services` / crate root so existing runtime public API paths remain available.
- Kept `SkillResolution`, `ExtensionReceipt`, skill service traits, prompt assembly, redaction, and executable runtime behavior in `clankers-runtime`; only reusable request/snippet record ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message skill_resolution_request_roundtrip_preserves_requested_order --lib
cargo test -p clanker-message resolved_skill_snippet_roundtrip_preserves_source --lib
cargo test -p clankers-runtime config_prompt_skill_service_fixtures_cover_host_desktop_missing_and_redaction --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
