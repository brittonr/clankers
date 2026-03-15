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
clankers auth login              # pick a provider
clankers auth login openai       # specific provider
clankers auth status             # check credentials
```

## Multiple accounts

```bash
clankers auth login --account work
clankers auth login --account personal
clankers auth switch work
clankers auth status
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
