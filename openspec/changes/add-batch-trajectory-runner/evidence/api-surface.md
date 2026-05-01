# Batch Trajectory Runner API Surface

## User-facing surface

The first pass exposes batch execution as an explicit foreground CLI command, not as a model-callable tool or hidden daemon behavior:

```text
clankers batch run --input <prompts.jsonl> --output <dir> [--concurrency <n>] [--format jsonl|sharegpt] [--resume]
```

Input is a local UTF-8 JSONL file. Each nonblank line is one prompt job object:

```json
{"id":"case-001","prompt":"Summarize @README.md"}
```

Supported first-pass fields:

- `id`: optional caller-provided stable identifier. If omitted, clankers derives a deterministic line-based id.
- `prompt`: required prompt text.
- `metadata`: optional JSON object copied to result metadata after redaction/normalization.

Output is a local directory. The first pass writes deterministic local files:

- `results.jsonl`: one structured result per input job.
- `trajectory.jsonl` or `sharegpt.jsonl`: export format selected by `--format`.
- `run.json`: normalized run metadata such as source path, output path, concurrency, counts, status, start/end timestamps, and first error summary.

## Policy boundaries

- The command is foreground and bounded. It does not daemonize, schedule, upload, or spawn unbounded work.
- `--concurrency` defaults conservatively and is capped by implementation policy.
- `--resume` skips already completed job ids found in the output directory and records skipped/completed counts.
- Input and output paths are local filesystem paths resolved relative to the current working directory.
- Existing provider/model/account CLI settings continue to own provider selection; the batch API should not introduce new credential syntax.

## Unsupported first-pass cases

The first pass returns actionable unsupported errors for:

- Remote dataset URLs and network fetches.
- Recursive directory/glob expansion as batch input.
- Background daemon scheduling or detached batch execution.
- TUI live batch dashboards and attach-time controls.
- Training/eval platform uploads.
- Arbitrary tool-call trajectory export beyond the normalized text/session data supported by clankers session exports.
- Unbounded concurrency or output paths that cannot be created safely.

## Implementation-facing API shape

A small reusable batch module should define:

- `BatchRunConfig`: input path, output directory, concurrency, format, resume flag, cwd.
- `BatchJob`: id, prompt, metadata, source line.
- `BatchJobResult`: id, status, response text or normalized error, timing, metadata.
- `BatchRunSummary`: counts, output file paths, status, elapsed time.
- `TrajectoryFormat`: `jsonl` and `sharegpt` in the first pass.

Command handling can validate and parse these types without contacting providers, enabling fast unit tests for policy boundaries before the execution backend is wired.
