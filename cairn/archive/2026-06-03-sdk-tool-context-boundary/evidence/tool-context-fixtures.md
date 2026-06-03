# Tool context neutral-service fixture evidence

Evidence-ID: sdk-tool-context-boundary-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: sdk-tool-context-boundary.verification.fixtures,sdk-tool-context-boundary.neutral-services.representative-tools,sdk-tool-context-boundary.neutral-services.missing-service
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers grep::tests::neutral_context_search
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers neutral_local_search
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent concrete_controller_services_expose_db_memory_search_service
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tool_context_migration_inventory
```

## Relevant output

```text
grep::tests::neutral_context_search
PASS clankers tools::grep::tests::neutral_context_search_emits_progress_without_legacy_tool_context
PASS clankers tools::grep::tests::neutral_context_search_fails_closed_without_progress_service
PASS clankers tools::grep::tests::neutral_context_search_respects_cancellation
PASS clankers tools::grep::tests::neutral_context_search_respects_capability_denial
Summary: 4 tests run: 4 passed, 1546 skipped

neutral_local_search
PASS clankers tools::external_memory::tests::neutral_local_search_fails_closed_without_search_service
PASS clankers tools::external_memory::tests::neutral_local_search_uses_injected_search_service
Summary: 2 tests run: 2 passed, 1546 skipped

concrete_controller_services_expose_db_memory_search_service
PASS clankers-agent turn::execution::tests::concrete_controller_services_expose_db_memory_search_service
Summary: 1 test run: 1 passed, 194 skipped

tool_context_migration_inventory
PASS clankers tools::tool_context_migration_tests::tool_context_migration_inventory_covers_service_families_and_representatives
Summary: 1 test run: 1 passed, 1547 skipped
```

## Coverage notes

The representative search path is `external_memory` local search through `ToolSearchService` / `ToolInvocationContext`. The representative progress path is `grep` through `ToolProgressSink` / `ToolInvocationContext`, with missing-progress, cancellation, and capability-denial fail-closed coverage. Agent controller services now expose a DB-backed memory search service as the desktop compatibility adapter for neutral local search.
