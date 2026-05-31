# Proposal: Harden Steel Substrate Checker Paths

## Summary

Keep the Steel tool/plugin/subagent substrate checker runnable after its Cairn change is archived by resolving active-change paths first and falling back to archived tasks plus the canonical specification.

## Motivation

`scripts/check-steel-tool-plugin-substrate.rs` currently reads `cairn/changes/steel-tool-plugin-substrate/...` directly. That active change has been archived, so the checker fails before validating the runtime substrate contract or writing its receipt. The checker should remain a durable regression rail after archive.

## Scope

- Update the checker path constants and artifact hashing to use the active change when present.
- Fall back to `cairn/archive/1970-01-01-steel-tool-plugin-substrate/tasks.md` for completed task traceability.
- Fall back to `cairn/specs/steel-tool-plugin-substrate/spec.md` for the authoritative canonical spec.
- Preserve the existing substrate markers, redaction checks, receipt output path, and hashed artifact receipt.

## Non-Goals

- Changing Steel-mediated dispatch behavior.
- Expanding the checker to new executor kinds.
- Reopening or rewriting the archived Steel substrate change.
