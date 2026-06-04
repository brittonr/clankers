# Design: Classify Runtime SDK Facade

## Context

The SDK tutorial says `clankers-runtime` is a Rust-facing embedding boundary, while product guidance keeps green generic SDK crates limited to message/engine/engine-host/tool-host/adapters/core. Runtime contains useful neutral DTOs but also many extension service, process, prompt, auth, and dynamic runtime surfaces that are intentionally app-edge.

## Decisions

### 1. Classification comes before promotion

Do not keep growing runtime as an implicit SDK. First decide whether it remains yellow-only, exposes a documented green subset, or splits green DTOs into smaller crates.

### 2. Public runtime API must be inventoried

Replace the hardcoded `public_type_names()` boundary guard with a deterministic inventory over exported runtime items, stability labels, and forbidden dependency/source tokens.

### 3. Defaults remain fail-closed

Any runtime surface that requires provider/auth/plugin/process/prompt filesystem/desktop state must require explicit host injection. Missing services must fail closed without ambient global lookup.

## Risks / Trade-offs

- A full runtime split could be large; the first change should create evidence and classification before broad refactors.
- Too many yellow APIs can dilute SDK guidance; docs must state what embedders can rely on.
- Inventory labels may expose existing accidental exports; classify before hiding or promoting.
