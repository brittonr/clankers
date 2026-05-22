# Proposal: Steel Self-Mutation Policy

## Why

The Steel Lisp runtime proposal gives Clankers a constrained embedded scripting layer, and the self-evolution control spec already supports isolated candidate generation plus explicit application. The remaining design question is direct mutation of skills, prompts, and code: useful for fast self-improvement loops, but dangerous if Steel receives ambient filesystem, process, credential, or provider authority.

Nickel and UCAN are the right split for this boundary. Nickel should own declarative mutation policy that is reviewable, typed, exported, and hashable. UCAN should carry the runtime authority to exercise a narrow mutation verb against a bounded target. Steel may plan or request mutation, but Rust remains the enforcement and mutation authority.

## What Changes

- Add a `steel-self-mutation-policy` capability spec that defines live self-modification as an explicit opt-in capability, separate from default self-evolution runs.
- Define Nickel-authored policy for mutation target classes, path scopes, host-function verbs, runtime profiles, approval tiers, preflight requirements, verification gates, and rollback requirements.
- Define UCAN runtime authorization checks for mutation verbs and resources, including expiry, delegation scope, revocation/denial handling, and receipt-safe metadata.
- Define typed Steel host functions for requesting mutation without granting ambient filesystem/process/network authority.
- Require deterministic receipts with Nickel policy hash, UCAN-safe metadata, target hashes, approval/preflight/verification outcomes, and rollback evidence.
- Require positive and negative fixtures for allowed mutation, denied path escape, expired or missing UCAN, failed verification, raw Steel filesystem denial, and rollback guards.

## Scope

In scope:

- Steel-requested live mutation of explicitly allowed skills, prompts, tool descriptions, and code paths.
- Nickel policy schema/export/check expectations.
- UCAN ability/resource mapping for mutation verbs.
- Host-function contracts and Rust enforcement boundaries.
- Receipt, audit, rollback, and verification requirements.

Out of scope:

- Giving Steel raw filesystem, shell, network, credential, provider, daemon, or git authority.
- Replacing the conservative isolated-candidate self-evolution flow.
- Silent or automatic promotion without an explicit opt-in command/capability.
- Treating Nickel policy hashes as authorization by themselves.
- Embedding UCAN secrets or compact tokens in receipts.

## Impact

- Extends `steel-lisp-runtime` with a deliberate live mutation capability that keeps Steel as planner/requester and Rust as authority.
- Extends self-evolution concepts with an explicit alternative to isolated candidate application, without weakening the default no-live-mutation rule.
- Introduces a policy/export/check rail likely under a repo-owned `policy/` subtree, plus tests that prove Nickel/UCAN denial paths fail closed.
