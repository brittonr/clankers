Evidence-ID: checker-validation
Artifact-Type: validation-log
Task-ID: V2
Covers: r[ucan-basalt-daemon-auth.verification.concrete-file-path-checker]
Status: pass

# Checker Validation

## Command

`TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-ucan-concrete-file-tool-paths.rs`

## Result

Pass. The checker wrote `target/ucan-concrete-file-tool-paths/receipt.json`.

## Receipt Summary

The receipt schema is `clankers.ucan_concrete_file_tool_paths.receipt.v1`. It hashes only source/lifecycle artifacts and records these validation surfaces:

- `public-ucan-file-tool-omitted-path-denial`
- `public-ucan-file-tool-blank-path-denial`
- `concrete-file-request-construction`
- `legacy-local-gate-non-regression`

The receipt redaction block records that raw compact UCAN tokens, signing keys, prompts, provider payloads, file contents, and tool input bodies are not embedded.
