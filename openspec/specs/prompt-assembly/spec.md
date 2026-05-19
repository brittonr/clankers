# prompt-assembly Specification

## Purpose
TBD - created by archiving change extract-prompt-assembly-service. Update Purpose after archive.
## Requirements
### Requirement: Prompt assembly service [r[prompt-assembly.service]]

The system MUST provide a reusable prompt assembly service that builds system/user prompt context from explicit host policy, host-provided context, and optionally enabled Clankers discovery sources without depending on CLI, TUI, daemon, or provider request types. Prompt assembly MUST be repeatable for multiple prompts in one session and MUST return prompt data to the caller without mutating session busy state, follow-up state, or model-turn completion state.

#### Scenario: repeated prompt assembly does not suppress execution [r[prompt-assembly.service.repeated-prompt-no-suppression]]

- GIVEN a session has already assembled, dispatched, streamed, and completed one prompt
- WHEN a later prompt is assembled in the same session
- THEN prompt assembly MUST return the assembled prompt and safe provenance for that later prompt
- THEN it MUST NOT decide that the later prompt is already completed because prior prompt metadata exists
- THEN shell/controller code MUST still submit the later assembled prompt through the normal accepted-prompt lifecycle

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

### Requirement: Prompt assembly kit emits deterministic provenance evidence [r[prompt-assembly.prompt-assembly-kit]]

The system MUST define `prompt-assembly-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, and safe evidence.

#### Scenario: Brick boundary is explicit [r[prompt-assembly.prompt-assembly-kit.boundary]]

- GIVEN a product or contributor adopts the `prompt-assembly-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name which behavior is reusable, which behavior stays product-owned, and which shell/runtime systems are out of scope
- THEN the brick MUST NOT silently depend on ambient credentials, daemon sessions, TUI state, provider discovery, plugin supervision, Matrix, iroh, or global singleton runtime state unless the design explicitly labels that path as app-edge

#### Scenario: Brick has executable evidence [r[prompt-assembly.prompt-assembly-kit.evidence]]

- GIVEN the brick is changed
- WHEN the focused verification for the change runs
- THEN it MUST exercise at least one positive path and one fail-closed or negative path through deterministic fixtures, examples, policy checks, generated inventory checks, or receipt validation
- THEN evidence MUST be safe to commit or summarize without raw prompts, credentials, authorization headers, OAuth tokens, provider payloads, hidden context, raw tool arguments, or secret environment values

#### Scenario: Brick drift is diagnosable [r[prompt-assembly.prompt-assembly-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN the brick validation rail runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and receipt or fixture evidence together

### Requirement: Context-reference policy boundary [r[prompt-assembly.context-reference-boundary]]

The service MUST keep context-reference expansion behind explicit host policy and MUST report unsupported references as structured receipts rather than silently fetching or dropping them.

#### Scenario: references disabled for embedding [r[prompt-assembly.context-reference-boundary.disabled]]

- GIVEN an embedding host disables context-reference expansion
- WHEN user prompt text contains local file, directory, image, URL, diff, or session-style references
- THEN the service leaves the prompt text unexpanded or returns explicit unsupported metadata according to host policy
- THEN it does not perform filesystem, git, network, or session reads implicitly

### Requirement: Prompt assembly matrix participation [r[prompt-assembly.shell-adapter-parity-matrix]]
The system MUST verify prompt assembly as a host-owned input to shell adapter parity rather than as a hidden dependency of the reusable engine.

#### Scenario: prompt source varies without engine dependency [r[prompt-assembly.shell-adapter-parity-matrix.prompt-source]]
- GIVEN shell parity matrix cases include empty, static, project-context, OpenSpec-context, and host-supplied prompt sources where supported
- WHEN the shell adapter submits accepted work to the engine
- THEN the engine receives already-prepared prompt data
- THEN the engine does not read project files, skills, OpenSpec context, or prompt templates directly
