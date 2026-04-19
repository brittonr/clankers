# Authentication

## API keys

Set a provider key directly:

```bash
export ANTHROPIC_API_KEY=sk-...
export OPENAI_API_KEY=sk-...
```

## OAuth

Interactive login:

```bash
clankers auth login                                 # Anthropic default OAuth login
clankers auth login --provider openai-codex         # ChatGPT Plus/Pro Codex subscription login
clankers auth login --provider openai-codex --account work
clankers auth status --all                          # grouped provider status
```

`openai-codex` is separate from API-key `openai`.
Use `openai-codex/gpt-5.3-codex` for ChatGPT subscription Codex models and
`openai/gpt-4o` for API-key OpenAI models.
Unsupported `openai-codex` plans stay authenticated but unavailable for Codex use, and explicit or resumed
Codex requests fail closed instead of falling back to API-key `openai`.

## Multiple accounts

```bash
clankers auth login --account work
clankers auth login --provider openai-codex --account personal
clankers auth switch work
clankers auth switch --provider openai-codex personal
clankers auth status --provider openai-codex
```

## Capability tokens

UCAN-based tokens for scoping access to daemon sessions:

```bash
clankers token create --read-only
clankers token create --tools "read,grep,bash" --expire 24h
clankers token create --root
clankers token list
clankers token revoke <hash>
```
