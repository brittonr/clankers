Artifact-Type: implementation-validation-evidence
Task-ID: I1,I2,I3,V1
Covers: remaining-coupling-drain.runtime-fail-closed-defaults.prompt-skill-host-injection
Status: complete

## Reviewed-Evidence

Prompt/skill service split:

- `crates/clankers-runtime/src/prompt.rs` and `crates/clankers-runtime/src/services.rs` carry runtime prompt/skill service contracts as host-injected runtime surfaces.
- `crates/clankers-config` owns display-neutral config/prompt/skill service core behavior, validated by `config_core_services_are_display_neutral`.
- Root desktop adapters wire explicit prompt/skill services; missing runtime services fail closed without ambient desktop discovery.
- `policy/embedded-lego/runtime-facade-boundary.json` classifies `runtime-prompt-services` as `yellow-app-edge-prompt-skill-services` with `explicit-host-injection-required`.

Commands run:

```text
scripts/check-config-prompt-skill-services.rs
config::core::tests::config_core_services_are_display_neutral ... ok
clankers_runtime::tests::config_prompt_skill_service_fixtures_cover_host_desktop_missing_and_redaction ... ok
clankers_runtime::tests::prompt_source_service_injection_is_used_by_runtime_assembly ... ok
runtime_services::tests::desktop_runtime_skill_service_resolves_explicit_roots_without_content_leaks ... ok
config-prompt-skill-services receipt written to target/embedded-sdk-release/config-prompt-skill-services-receipt.json

scripts/check-runtime-facade-boundary.rs
ok: runtime facade boundary inventories clankers-runtime public API and dependency classifications
```

## Decision

Prompt and skill behavior is a host-injected yellow runtime service. Desktop `.clankers` / `.pi` / project-root discovery stays in desktop adapters and must not become runtime defaults.

## Follow-Up

If neutral prompt/skill DTOs are later promoted to green SDK crates, add a fixture-backed owner row and rerun the embedded SDK acceptance rail.
