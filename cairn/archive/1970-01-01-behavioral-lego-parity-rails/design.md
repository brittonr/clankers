# Design: Behavioral Lego Parity Rails

## Summary

Architecture rails should fail because behavior diverged, not only because a string disappeared. Freshness checks can remain as guardrails, but every claimed SDK/Lego boundary should have an executable fixture, typed manifest, AST dependency check, or receipt verifier with actionable diagnostics.

## Decisions

### Decision: each rail has an executable owner

For every matrix/freshness script, define the owner test or fixture it must execute or verify. Symbol checks may only confirm wiring to that owner.

### Decision: receipts name axes and outcomes

Behavioral receipts should include case id, axis values, expected outcome, observed outcome, source artifacts, and sanitized hashes. Failures should identify the boundary owner and requirement id.

### Decision: negative fixtures are first-class

Provider/auth disabled defaults, missing stores, absent plugins, denied capabilities, event redaction, and transport leakage must have negative/fail-closed cases, not only positive examples.

## Verification Plan

- Inventory current freshness scripts and classify each as executable, receipt verifier, AST rail, or temporary string check.
- Convert high-risk string checks to executable fixtures first: runtime extension service matrix, shell adapter parity matrix, event projection, provider service contract, session resume, and tool context.
- Wire receipt verification into the embedded SDK acceptance bundle and Nix check surface where practical.
