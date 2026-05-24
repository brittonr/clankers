# Steel Eval Operator Docs Specification

## Purpose

Defines the `steel-eval-operator-docs` capability.

## Requirements

### Requirement: Default discovery documentation [r[steel-eval-operator-docs.default-discovery-doc]]

Clankers docs MUST tell operators that the safe `steel_eval` built-in is available under ordinary default settings.

#### Scenario: Operator finds default tool behavior
- GIVEN an operator reads the built-in tools or Steel runtime docs
- WHEN they search for `steel_eval`
- THEN they MUST find that the tool is default-published under the pure default profile

### Requirement: Authority boundary documentation [r[steel-eval-operator-docs.authority-boundary-doc]]

Steel eval operator docs MUST distinguish pure eval from host authority, mutation, and Steel turn planning.

#### Scenario: Docs deny ambient authority
- GIVEN `steel_eval` is documented as default-published
- WHEN the authority boundary is described
- THEN docs MUST state that the default profile has no ambient host functions, no session capabilities, and no mutation authority

### Requirement: Opt-out documentation [r[steel-eval-operator-docs.opt-out-doc]]

Steel eval operator docs MUST show the explicit setting that omits default publication.

#### Scenario: Operator finds opt-out
- GIVEN an operator wants to hide `steel_eval`
- WHEN they read the config or tool docs
- THEN docs MUST name `steelEval.enabled = false` as the opt-out path

### Requirement: Documentation verification [r[steel-eval-operator-docs.docs-verification]]

The documentation slice MUST be verified without broad unrelated product gates.

#### Scenario: Doc slice has focused verification
- GIVEN only docs/catalog lines change
- WHEN verification runs
- THEN targeted grep/read checks and `git diff --check` MUST pass
- AND a cheap doc-adjacent command SHOULD run when available
