# SOUL/Personality API Surface

## First-pass CLI surface

```text
clankers soul status [--json]
clankers soul validate [--soul <PATH>] [--personality <NAME>] [--json]
```

- `status` reports the first-pass local SOUL/personality policy boundary.
- `validate` checks a local SOUL file path and/or named personality preset without reading raw content into replay metadata or mutating the active session prompt.
- Omitted `--soul` defaults to discovery intent for `SOUL.md` in the current project context.
- Omitted `--personality` defaults to no preset overlay.

## First-pass Specialty tool

Expose `soul_personality` as a Specialty tool with actions:

- `status`
- `validate`

Inputs:

- `soul`: optional local path or discovery marker
- `personality`: optional safe preset name

The tool should return `ToolResult::details` with only normalized metadata: source, action, status, backend, soul input kind/label, personality label, supported flag, and sanitized error class/message.

## Config surface

No required config in the first pass. Local project files and explicit CLI/tool parameters are enough to validate the boundary. Future config may add preset directories, default personality names, and prompt composition policy once runtime prompt assembly is wired and tested.

## Supported first-pass cases

- Local SOUL file path validation (`SOUL.md`, `./SOUL.md`, `file:./SOUL.md`).
- Discovery intent for a project-local `SOUL.md`.
- Safe personality preset names made from ASCII alphanumerics plus `-`, `_`, or `.`.
- `status` / `validate` returning structured, user-visible results without changing the active system prompt.

## Explicitly unsupported first-pass cases

- Remote SOUL/personality URLs (`http://`, `https://`).
- Cloud/object-store presets (`s3://`, `cloud:`).
- Shell commands or process substitution as persona sources.
- Provider-hosted persona fetches.
- Persisting raw SOUL.md contents or full prompt text in session metadata.
- Automatic runtime mutation of live TUI/daemon prompts before a dedicated prompt-composition test seam is added.
- Personality names containing path separators, whitespace, control characters, or credential-like target strings.
