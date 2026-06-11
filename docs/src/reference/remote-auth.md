# Remote Auth: Public UCAN + Basalt

Remote daemon access uses **public UCAN** credentials for delegated authority and **Basalt policy** for admission. The remote surfaces are iroh QUIC attach, chat/RPC-style prompt entrypoints, and Matrix-triggered session prompts. Local legacy `clanker-auth` compatibility is not the remote verifier path.

This page shows command shapes and operator workflow. It intentionally uses placeholders such as `<REMOTE_IROH_PUBLIC_KEY>` and `<PARENT_UCAN_ENVELOPE>` instead of token bodies, signing keys, auth JSON, or user credentials.

## Model

- **Audience**: the remote daemon identity/public key that should accept the credential.
- **Public UCAN envelope**: the portable capability credential created by `clankers token create`.
- **Delegation**: a child credential minted from a parent envelope with narrower authority or shorter lifetime.
- **Basalt policy admission**: every remote session create, attach, prompt, and tool-use admission request is checked against policy before side effects.
- **Redacted receipts**: logs and receipts record safe metadata such as hashes, audiences, capabilities, and verdicts; they must not print raw token bodies or private key material.

## Discover the remote audience

On the daemon host, get the iroh identity that remote clients should target. The exact operational channel can be a daemon status command, deployment metadata, or an operator-to-operator handoff, but the value used in token creation must be the remote daemon's public identity.

```bash
clankers daemon status
```

Use that identity as `<REMOTE_IROH_PUBLIC_KEY>` when minting credentials.

## Create a root or scoped remote credential

Prefer the narrowest credential that supports the workflow. A read-only attach credential can inspect session state without write tools:

```bash
clankers token create --read-only --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
```

Allow a bounded tool set for a short-lived remote operator:

```bash
clankers token create --tools "read,grep,find" --session-manage --for <REMOTE_IROH_PUBLIC_KEY> --expire 8h
```

Create a root credential only for a bootstrap operator that will immediately delegate narrower child credentials:

```bash
clankers token create --root --delegate --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
```

Store the returned public UCAN envelope in the local auth store or the deployment secret store used by the remote client. Do not paste the envelope into tickets, chat logs, or docs.

## Delegate from a parent credential

Delegation lets an operator mint a child credential with narrower scope than the parent. The parent envelope is passed with `--from`; the child still targets the same remote daemon audience with `--for`.

```bash
clankers token create \
  --from <PARENT_UCAN_ENVELOPE> \
  --read-only \
  --for <REMOTE_IROH_PUBLIC_KEY> \
  --expire 2h
```

For a bot or automation account, scope commands and tools explicitly:

```bash
clankers token create \
  --from <PARENT_UCAN_ENVELOPE> \
  --tools "read,grep" \
  --bot-commands "prompt,status" \
  --for <REMOTE_IROH_PUBLIC_KEY> \
  --expire 6h
```

## Use the credential with remote attach

After the public UCAN envelope is installed for the client account, attach to the daemon over iroh QUIC:

```bash
clankers attach --remote <REMOTE_NODE_ID>
```

Attach and session-management commands are admitted through shared session admission request helpers and Basalt policy. If the credential is missing, expired, revoked, for a different audience, or outside policy, the request fails closed before session mutation.

## Chat/RPC and Matrix entrypoints

Remote chat/RPC-style prompts and Matrix bridge prompts use the same public UCAN + Basalt admission path as attach:

- install a public UCAN envelope for the remote client identity;
- target the daemon with the remote node id or the configured Matrix bridge account;
- keep bot credentials scoped with `--bot-commands`, `--tools`, and short `--expire` values;
- rely on receipts for safe hashes/verdicts, not raw prompt or credential material.

Matrix bridge deployments should store only the credential required for that bridge account. If a Matrix room or bot is compromised, revoke or rotate that credential without rotating unrelated operator credentials.

## Revocation, rotation, and redacted receipts

List issued credential records:

```bash
clankers token list
```

Revoke by the credential hash shown in the list or by the envelope value held in a secure local store:

```bash
clankers token revoke <TOKEN_HASH>
```

Rotate by minting a replacement before revoking the old credential:

```bash
clankers token create --read-only --for <REMOTE_IROH_PUBLIC_KEY> --expire 24h
clankers token revoke <OLD_TOKEN_HASH>
```

For suspected compromise, revoke the affected credential immediately, rotate child credentials delegated from it, and inspect redacted admission receipts for denied or unexpected attempts. Receipt text should identify the hash/audience/verdict class without exposing the full envelope or private key material.

## Basalt source boundary

Local Cargo development uses the sibling workspace path:

```toml
basalt = { path = "../basalt", default-features = false }
```

Nix builds do not rely on an ambient sibling checkout. `flake.nix` maps the same `../basalt` path through unit2nix `externalSources` to the pinned `OnixResearch/basalt` flake input. This keeps local development ergonomic while preserving a reproducible Basalt source for Nix evaluation.
