Evidence-ID: checker-validation
Artifact-Type: validation-log
Task-ID: V2
Covers: r[ucan-basalt-daemon-auth.verification.canonical-file-path-checker]
Status: pass

# Checker Validation

## Command

`TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-ucan-canonical-file-tool-paths.rs`

## Result

Pass. The checker wrote `target/ucan-canonical-file-tool-paths/receipt.json`.

## Receipt Summary

The receipt schema is `clankers.ucan_canonical_file_tool_paths.receipt.v1`. It hashes only source/lifecycle artifacts and records these validation surfaces:

- `public-auth-file-root-threading`
- `relative-file-path-root-resolution`
- `parent-traversal-denial`
- `absolute-file-path-resource-preservation`
- `legacy-local-gate-non-regression`

The receipt redaction block records that raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, and tool input bodies are not embedded.
