# Steel Scheme Runtime and Live Mutation

Clankers embeds Steel Scheme through a Clankers-owned Rust runtime wrapper. Steel is treated as a constrained embedded interpreter for trusted orchestration/request logic, not as ambient host authority and not as an OS/process sandbox.

## Runtime surfaces

- `clankers steel status` reports wrapper/profile availability without executing user Steel code.
- `clankers steel eval <source>` evaluates through the wrapper-owned request/receipt DTOs.
- `clankers steel run <file>` reads source from a file and uses the same wrapper.
- The agent-visible `steel_eval` built-in tool is published under ordinary default settings and evaluates through the same Rust-owned wrapper/receipt seam.

All surfaces use named runtime profiles. The default profile is deny-by-default and has zero ambient filesystem, process, git, network, provider, credential, daemon, TUI, environment, clock, or native-tool authority. Host-visible effects require explicit registered host functions, session capability approval, disabled-tool checks, profile budgets, and deterministic receipts.

## Default `steel_eval` tool

`steel_eval` is intended as the safe operator-visible default for pure Scheme evaluation. With missing config, Clankers publishes the tool using the pure default profile: no ambient host functions, no session capabilities, zero host-call budget, bounded source/output/step limits, and deterministic redacted receipts. This makes the tool discoverable without enabling Steel turn planning or live mutation.

To omit the tool explicitly, set:

```json
{
  "steelEval": {
    "enabled": false
  }
}
```

The opt-out removes `steel_eval` from the built-in tool catalog; it does not change CLI `clankers steel status|eval|run` behavior and does not grant authority to any other Steel surface.

`steel_eval` receipts are review metadata, not authority grants. Operators should check the top-level tool receipt status/issue code, selected profile id, redaction policy, output length, receipt hash, and nested runtime receipt. The default profile must not report host functions or session capabilities, and any future non-pure profile must be configured and authorized explicitly.

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
