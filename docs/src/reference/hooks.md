# Hooks

Clankers hooks can run shell scripts from `.clankers/hooks/` and can also be bridged to plugins. Script filenames are the kebab-case hook names below.

## Prompt and turn lifecycle

| Script filename | Behavior | Notes |
|---|---|---|
| `pre-prompt` | Blocking pre-hook; may deny or modify prompt text | Runs before the user message is appended. A JSON stdout object with `text` rewrites the prompt. |
| `post-prompt` | Observational post-hook | Runs after the prompt outcome is known; cannot rewrite the result. |
| `pre-turn` | Blocking pre-hook; deny only | Runs after context/model config is ready and before the first model request/tool loop. |
| `post-turn` | Observational post-hook | Runs once for the prompt-level agent turn with status, model, usage, counts, safe error, and prompt correlation metadata. |
| `turn-start` | Non-blocking model-turn notification | Existing lifecycle notification from model/transcript turn events; not a pre-turn gate. |
| `turn-end` | Non-blocking model-turn notification | Existing lifecycle notification from model/transcript turn events; not a post-turn outcome hook. |

Prompt and turn payloads include a stable `prompt_id`, a bounded `prompt_preview`, and a `prompt_digest`. Turn payloads do not expose the full system prompt or full conversation history.

## Tool, git, session, and model hooks

| Script filename | Behavior |
|---|---|
| `pre-tool` | Blocking pre-hook; may deny or modify tool input. |
| `post-tool` | Observational post-hook. |
| `pre-commit` | Blocking pre-hook; may deny or modify git metadata. |
| `post-commit` | Observational post-hook. |
| `session-start` | Observational lifecycle hook. |
| `session-end` | Observational lifecycle hook. |
| `on-error` | Observational lifecycle hook. |
| `model-change` | Observational lifecycle hook. |

## Plugin event mapping

Hooks exposed through the plugin event protocol use these event kinds:

| Hook script | Plugin event |
|---|---|
| `pre-prompt`, `post-prompt` | `user_input` |
| `pre-turn` | `pre_turn` |
| `post-turn` | `post_turn` |
| `turn-start` | `turn_start` |
| `turn-end` | `turn_end` |
| `pre-tool` | `tool_call` |
| `post-tool` | `tool_result` |
| `session-start` | `session_start` |
| `session-end` | `session_end` |
| `model-change` | `model_change` |

`pre-commit`, `post-commit`, and `on-error` do not currently have plugin event mappings.

## Configuration

Use `/hooks list` inside the TUI to see installed scripts, plugin mappings, and behavior labels. Set `hooks.disabledHooks` to any script filename (for example `pre-turn`) to disable that hook point.
