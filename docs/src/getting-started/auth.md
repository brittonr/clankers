# Authentication

## API keys

Set a provider key directly:

```bash
export ANTHROPIC_API_KEY=***
export OPENAI_API_KEY=***
clankers auth add openrouter --api-key sk-or-... --account backup
clankers auth list --all
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
Use `openai-codex/gpt-5.5` for ChatGPT subscription Codex models and
`openai/gpt-4o` for API-key OpenAI models.
Unsupported `openai-codex` plans stay authenticated but unavailable for Codex use, and explicit or resumed
Codex requests fail closed instead of falling back to API-key `openai`.

## Multiple accounts and credential pools

Multiple credentials for the same provider form a same-provider pool. Clankers tries the pool before model/provider fallback: a single 429 is retried before rotation, repeated 429s rotate for 1 hour, and a 402 billing/quota error rotates immediately for 24 hours.

```bash
clankers auth login --account work
clankers auth add anthropic --api-key sk-ant-... --account backup
clankers auth login --provider openai-codex --account personal
clankers auth switch work
clankers auth switch --provider openai-codex personal
clankers auth status --provider openai-codex
```

Pool selection defaults to `fill_first`. For in-process pools, set `CLANKERS_CREDENTIAL_POOL_STRATEGY` to `round_robin`, `least_used`, or `random` to change selection.

## Remote capability tokens

Remote daemon access uses public UCAN credentials plus Basalt policy admission. Legacy `clanker-auth` compatibility is local-only context; remote attach/chat/Matrix admission should use public UCAN envelopes targeted at the daemon audience. See [Remote Auth: Public UCAN + Basalt](../reference/remote-auth.md) for delegation, revocation, Matrix/chat storage, receipt redaction, and the Basalt source boundary.

```bash
clankers token create --read-only --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
clankers token create --tools "read,grep,bash" --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
clankers token create --root --delegate --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
clankers token list
clankers token revoke <TOKEN_HASH>
```
