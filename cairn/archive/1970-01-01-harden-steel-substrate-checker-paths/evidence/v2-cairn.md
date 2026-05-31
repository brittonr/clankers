Evidence-ID: harden-steel-substrate-checker-paths.V2.cairn
Task-ID: V2
Artifact-Type: command-output
Covers: steel-tool-plugin-substrate.checker-paths.active-archive-resolution, steel-tool-plugin-substrate.checker-paths.receipt-artifacts
Status: pass
Generated-By: pi
Generated-At: 2026-05-30

# Cairn Gate and Validation Evidence

## Commands and outputs

```text
nix run .#cairn -- gate proposal harden-steel-substrate-checker-paths --root .
```

```json
{
  "stage": "proposal",
  "valid": true,
  "verdict": "PASS",
  "issues": []
}
```

```text
nix run .#cairn -- gate design harden-steel-substrate-checker-paths --root .
```

```json
{
  "stage": "design",
  "valid": true,
  "verdict": "PASS",
  "issues": []
}
```

```text
nix run .#cairn -- gate tasks harden-steel-substrate-checker-paths --root .
```

```json
{
  "stage": "tasks",
  "valid": true,
  "verdict": "PASS",
  "issues": []
}
```

```text
nix run .#cairn -- validate --root .
```

```json
{
  "changes": 1,
  "valid": true,
  "change_issues": [],
  "spec_issues": [],
  "specs_validated": 51
}
```

```text
git diff --check
```

No whitespace errors were reported.

## Post-archive smoke

After `nix run .#cairn -- archive harden-steel-substrate-checker-paths --root . --execute`, the durable rail still passed:

```text
./scripts/check-steel-tool-plugin-substrate.rs
```

```text
steel tool/plugin/subagent substrate receipt written to target/steel-tool-plugin-substrate/receipt.json
```

```text
nix run .#cairn -- validate --root .
```

```json
{
  "changes": 0,
  "valid": true,
  "change_issues": [],
  "spec_issues": [],
  "specs_validated": 50
}
```
