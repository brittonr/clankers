Artifact-Type: validation-log
Task-ID: I25,V24
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved reusable runtime prompt assembly DTOs to neutral message contracts:

- Added `clanker_message::PromptAssemblyPolicy`, `PromptSourceRequest`, `PromptSources`, `HostContext`, `SkillSnippet`, `AssembledPrompt`, `PromptSection`, `PromptProvenance`, `PromptSourceKind`, `ContextReferenceRequest`, `ContextReferenceKind`, and `UnsupportedContextReference`.
- Re-exported those DTOs through `clankers-runtime::prompt` and the runtime crate root so existing runtime public API paths remain available.
- Kept `PromptAssembler`, prompt source services, model adapters, prompt/session identities, fail-closed disabled prompt service behavior, and redaction/assembly execution in `clankers-runtime`; only serde-friendly prompt data records moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message prompt_assembly_policy_preserves_host_defaults --lib
cargo test -p clanker-message prompt_sources_roundtrip_preserves_context_references_and_defaults --lib
cargo test -p clanker-message assembled_prompt_roundtrip_preserves_provenance_and_unsupported_refs --lib
cargo test -p clankers-runtime config_prompt_skill_service_fixtures_cover_host_desktop_missing_and_redaction --lib
cargo test -p clankers-runtime prompt_assembly_reports_disabled_context_references_without_content --lib
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
