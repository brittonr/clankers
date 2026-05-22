# Steel Scheme Runtime and Live Mutation

Clankers embeds Steel Scheme through a Clankers-owned Rust runtime wrapper. Steel is treated as a constrained embedded interpreter for trusted orchestration/request logic, not as ambient host authority and not as an OS/process sandbox.

## Runtime surfaces

- `clankers steel status` reports wrapper/profile availability without executing user Steel code.
- `clankers steel eval <source>` evaluates through the wrapper-owned request/receipt DTOs.
- `clankers steel run <file>` reads source from a file and uses the same wrapper.

All surfaces use named runtime profiles. The default profile is deny-by-default and has zero ambient filesystem, process, git, network, provider, credential, daemon, TUI, environment, clock, or native-tool authority. Host-visible effects require explicit registered host functions, session capability approval, disabled-tool checks, profile budgets, and deterministic receipts.

## Mutation-capable runs

Steel-requested live mutation is separate from default Steel eval and separate from isolated self-evolution candidate runs. A mutation-capable run must be named and visible to the operator/session observer and must carry:

- mutation profile name,
- target class and normalized target resource,
- intended verb such as propose/apply/commit/rollback,
- approval reference/tier,
- receipt destination,
- Nickel policy hash,
- safe UCAN metadata only.

Nickel declares target classes, path scopes, verbs, approvals, verification profiles, runtime budgets, redaction policy, and rollback requirements. UCAN-style grants authorize runtime abilities/resources. Rust enforces policy and authority, performs preflight/apply/rollback, runs verification, and emits receipts.

## Receipt and rollback review

Receipts may include stable status, issue codes, target metadata, policy hash, safe UCAN metadata, before/after hashes, backup hashes, verification outcome, and redaction decisions. Receipts must not include raw UCAN proofs, compact tokens, credentials, provider payloads, oversized patch bodies, or uncontrolled absolute-path dumps.

Rollback is guarded: Rust verifies that the current target hash still matches the recorded post-apply hash and that backup bytes match the recorded backup hash before restoring. If an operator edited the target after mutation, rollback fails closed instead of clobbering that edit.

## Security wording

Steel support should be described as a constrained embedded interpreter with host-function gates and Rust enforcement. Do not claim VM, process, or OS-level sandbox isolation unless a separate isolation proof exists.
