# prompt-assembly Specification

## Purpose
TBD - created by archiving change extract-prompt-assembly-service. Update Purpose after archive.
## Requirements
### Requirement: Prompt assembly service [r[prompt-assembly.service]]

The system MUST provide a reusable prompt assembly service that builds system/user prompt context from explicit host policy, host-provided context, and optionally enabled Clankers discovery sources without depending on CLI, TUI, daemon, or provider request types.

#### Scenario: host supplies all context [r[prompt-assembly.service.host-context-only]]

- GIVEN an embedding host disables Clankers filesystem/project discovery and supplies app-native system instructions and prompt context
- WHEN prompt assembly runs
- THEN the assembled prompt contains only the host-provided sections and built-in runtime invariants explicitly enabled by policy
- THEN no AGENTS.md, CLAUDE.md, SOUL.md, OpenSpec, skill, or project context files are read from disk

#### Scenario: Clankers discovery parity [r[prompt-assembly.service.clankers-parity]]

- GIVEN normal Clankers CLI/TUI/daemon prompt discovery policy
- WHEN prompt assembly runs through the service
- THEN section precedence and inclusion match the existing lifecycle contract for settings, SYSTEM/APPEND_SYSTEM, SOUL/personality, AGENTS/CLAUDE, context files, specs, skills, learning guidance, and suffixes

### Requirement: Prompt assembly provenance [r[prompt-assembly.provenance]]

The prompt assembly service MUST return safe provenance metadata for included, skipped, unsupported, and failed prompt sections.

#### Scenario: provenance omits raw hidden context [r[prompt-assembly.provenance.redacted]]

- GIVEN prompt assembly reads local project guidance, skills, SOUL/personality, or host-provided context
- WHEN provenance is returned to the host
- THEN metadata includes safe source kind, precedence, status, byte counts, content hashes, optional preset ids, and sanitized errors
- THEN metadata MUST NOT include raw hidden prompt text, credential-bearing URLs, environment values, headers, or full paths unless the host explicitly supplied that data as visible prompt content

### Requirement: Context-reference policy boundary [r[prompt-assembly.context-reference-boundary]]

The service MUST keep context-reference expansion behind explicit host policy and MUST report unsupported references as structured receipts rather than silently fetching or dropping them.

#### Scenario: references disabled for embedding [r[prompt-assembly.context-reference-boundary.disabled]]

- GIVEN an embedding host disables context-reference expansion
- WHEN user prompt text contains local file, directory, image, URL, diff, or session-style references
- THEN the service leaves the prompt text unexpanded or returns explicit unsupported metadata according to host policy
- THEN it does not perform filesystem, git, network, or session reads implicitly
