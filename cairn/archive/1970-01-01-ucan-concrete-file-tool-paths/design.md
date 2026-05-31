# Design: UCAN Concrete File Tool Paths

## Context

`PublicUcanCapabilityGate` translates remote public UCAN tool calls into one or more `BasaltAdmissionRequest` values before a tool executes. The generic tool request uses `clankers:tool/<name>` with `tool/use`; file-oriented tools add `file/read` or `file/write` requests when a `path` is present.

`grep`, `find`, and `ls` can omit `path` and default to the process working directory. That default is convenient locally, but it is not an explicit normalized resource for remote public UCAN admission. The accepted UCAN daemon-auth vocabulary requires concrete invocation requests and keeps wildcard/default semantics in grants or caveats rather than in the invocation.

## Decisions

### 1. Fail closed before Basalt when file path is missing

**Choice:** Add a small helper in `src/capability_gate.rs` that returns a non-empty `path` for file read/write tools or a typed denial string when no concrete path was provided.

**Rationale:** This keeps request construction deterministic and prevents the authorization layer from fabricating an ambient resource such as the current working directory. It also avoids invoking Basalt with incomplete request facts.

### 2. Keep public UCAN behavior narrower than legacy local capability settings

**Choice:** Apply the concrete-path requirement only in `public_tool_requests`, the public UCAN + Basalt path. Leave `UcanCapabilityGate` unchanged for local `settings.defaultCapabilities` compatibility.

**Rationale:** The hardening targets remote public UCAN admission semantics. Local settings examples currently use lightweight tool-name filters and should not gain new path requirements in this slice.

### 3. Verify with unit tests plus a source/receipt checker

**Choice:** Add focused tests for omitted and blank paths on public file tools, plus a deterministic checker that verifies the helper, tests, Cairn spec, archived tasks, and safe receipt output.

**Rationale:** The checker gives durable evidence that the path requirement remains tied to the public UCAN gate and that lifecycle closeout leaves a reviewable receipt without raw file contents, prompts, credentials, or token material.

## Risks / Trade-offs

- Remote public UCAN sessions that previously invoked `grep`, `find`, or `ls` without a path will now receive a denial and must pass an explicit path.
- The existing resource normalizer still uses the path string supplied by the tool input; broader canonicalization of relative paths is intentionally out of scope for this small hardening slice.
