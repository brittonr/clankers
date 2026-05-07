## Context

Request lifecycle docs show prompt assembly pulls from settings prefix/suffix, SYSTEM/APPEND_SYSTEM, SOUL/personality, AGENTS.md/CLAUDE.md, context files, specs, skills, and learning guidance. Embedders may want this, but only as an explicit policy choice.

## Decisions

### PromptAssembler service

**Choice:** Create a reusable prompt assembly service below CLI/TUI/daemon shells and above engine/provider request construction.

**Rationale:** The engine should accept prepared prompts; prompt assembly is a host/app policy layer.

### Policy-controlled discovery

**Choice:** Filesystem, project, OpenSpec, skills, SOUL/personality, and context-reference discovery must be individually policy-controlled.

**Rationale:** Host apps need reproducible and sandboxed context assembly.

### Safe provenance metadata

**Choice:** Return section labels, precedence, byte counts, hashes, statuses, and sanitized errors, not raw hidden file contents or full secret-bearing paths.

**Rationale:** Hosts need auditability without leaking private prompt material.

## Risks / Trade-offs

- **Parity drift:** Prompt order is subtle. Add fixtures that compare current Clankers assembly to service output.
- **Overexposure:** Provenance can leak paths. Use basename/path-hash style metadata.
- **Feature creep:** URL/session-artifact/context-reference expansion should stay governed by existing policies and not become implicit network fetching.
