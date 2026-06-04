## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for `expand-context-references`.
- [x] Validate the OpenSpec package with `openspec validate expand-context-references --strict` and record any follow-up findings.

## Phase 2: Implementation

- [x] Inventory current `context-references` code/docs seams and record the exact files to touch. Evidence: `verification.md` inventory covers resolver, integration tests, README, and lifecycle docs.
- [x] Add typed policy/config/request/receipt models with unit tests. Evidence: `ContextReferencePolicy`, expanded `ContextReferenceKind`, safe `ContextReferenceMetadata`, and `clankers-util` `at_file` tests.
- [x] Implement the first runtime/adapter slice behind deterministic fake tests. Evidence: bounded `@diff` expansion and local fake-HTTP URL fetch tests in `crates/clankers-util/src/at_file.rs` and `tests/context_references.rs`.
- [x] Wire the feature through the shared clankers surface without bypassing daemon/session/tool policy. Evidence: existing `expand_at_refs_with_images` call sites now get diff support and fail-closed URL receipts by default; policy-enabled URL fetch uses explicit `expand_at_refs_with_policy`.
- [x] Update README and relevant docs for supported behavior, non-goals, and safety policy. Evidence: README and `docs/src/reference/request-lifecycle.md` updated.

## Phase 3: Verification and Closeout

- [x] Run targeted package/integration checks for the touched modules. Evidence: `cargo test -p clankers-util at_file` and `cargo test --test context_references` passed.
- [x] Run `cargo check --tests` for affected crates. Evidence: `CARGO_TARGET_DIR=target cargo check --tests` passed.
- [x] Run `git diff --check`. Evidence: `git diff --check` passed.
- [x] Sync the delta spec into the canonical `context-references` spec and archive the change after implementation tasks complete. Evidence: `openspec archive expand-context-references --yes` updated canonical `context-references`; `openspec validate context-references --strict` passed.
