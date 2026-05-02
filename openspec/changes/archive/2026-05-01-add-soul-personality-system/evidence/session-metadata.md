# SOUL/Personality Replay Metadata Boundary

## Scope

The first-pass SOUL/personality surface is validation-only. It records replay/debug metadata through `ToolResult::details` from `src/tools/soul_personality.rs` using the safe `SoulValidation` structure from `src/soul_personality.rs`.

## Persisted safe fields

The details payload may include only normalized validation metadata:

- `source`: fixed marker `soul_personality`.
- `backend`: fixed first-pass backend label for local validation.
- `soul_kind`: normalized source class such as `discover`, `local_file`, `remote`, `cloud`, or `command`.
- `soul_label`: safe display label such as `SOUL.md`, `https`, `cloud`, or `command`; local paths are reduced to the file name.
- `personality`: optional validated preset identifier.
- `supported`: boolean policy result.
- `error_kind` / `error_message`: safe unsupported or validation category/message.

## Explicitly excluded data

The first pass must not persist or log:

- raw `SOUL.md` contents or personality prompt contents;
- full local paths beyond the safe file-name label;
- remote persona URLs, webhook URLs, query strings, headers, or payloads;
- shell commands, environment values, command output, or hook payloads;
- credentials, tokens, API keys, passwords, authorization headers, cookies, or connection strings;
- encrypted/secret bundle material or autonomous self-modification traces.

Unsupported remote/cloud/command/persona-hook cases fail before any network, shell, provider, or prompt-mutation side effect.

## Verification

`cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers soul --no-fail-fast` passed after the CLI/tool adapter landed. The tool tests assert safe details for both a supported local path and an unsupported remote source, including that secret-like URL material is not preserved in output.
