# Design: Remote Auth UX Docs

## Context

Daemon auth now uses public UCAN credential envelopes plus Basalt policy admission for remote session, prompt, attach, and call-time tool gates. The implementation already rejects legacy `clanker-auth` credentials by default and keeps redacted receipts. The remaining gap is operator comprehension: docs should show the safe workflow without making users inspect source or lifecycle archives.

## Decisions

### 1. Author one reference page and link it from shorter guides

**Choice:** Add a dedicated remote-auth reference page for the full model, then keep README and getting-started docs as short entrypoints.

**Rationale:** Token creation, delegation, remote attach, Matrix/chat storage, Basalt policy, revocation, and redaction are too dense for the README. A single reference page avoids duplicate examples while allowing concise cross-links.

### 2. Use public UCAN + Basalt terminology explicitly

**Choice:** User-facing text must say "public UCAN" and "Basalt policy" for remote daemon access, and reserve "legacy `clanker-auth`" wording for local compatibility context.

**Rationale:** The old docs implied generic UCAN tokens over `clanker-auth`; that conflicts with the accepted `ucan-basalt-daemon-auth` spec and can send operators toward the wrong verifier path.

### 3. Keep command examples clap-accurate and secret-safe

**Choice:** Examples must use real command shapes such as `clankers token create --read-only --for <REMOTE_IROH_PUBLIC_KEY>` and `clankers attach --remote <REMOTE_NODE_ID>`, but placeholders must never include actual compact UCAN token strings, signing keys, auth JSON, or user credentials.

**Rationale:** Auth docs are security-sensitive and easy to cargo-cult. Examples need to be executable shapes while leaving token bodies out of docs, logs, and receipts.

### 4. Keep Basalt as the workspace path plus Nix external source

**Choice:** Continue using `basalt = { path = "../basalt" }` for local Cargo development and the existing flake `externalSources` mapping to the pinned `OnixResearch/basalt` input for Nix builds.

**Rationale:** This matches the current workspace graph and avoids vendoring, stale generated copies, or a second Basalt source. The docs should explain that local `../basalt` is the developer path while Nix supplies the reproducible pin.

## Risks / Trade-offs

- Docs can drift from clap flags and protocol seams unless a focused source/docs contract test checks stable examples.
- Overly detailed examples could leak implementation internals or encourage users to paste raw tokens into logs; keep placeholders and redaction guidance explicit.
- Matrix/chat auth storage behavior is less obvious than `attach --remote`; docs should describe it as stored public credential lookup without promising a separate UX not yet implemented.
