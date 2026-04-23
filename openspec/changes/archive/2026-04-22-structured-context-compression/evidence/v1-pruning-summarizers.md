Artifact-Type: verification-note
Evidence-ID: v1-pruning-summarizers
Task-ID: V1
Covers: structured.compaction.pruning.prepass.bashtoolresult, structured.compaction.pruning.prepass.readtoolresult, structured.compaction.pruning.prepass.commonfiletools, structured.compaction.pruning.prepass.subagenttoolresult, structured.compaction.pruning.prepass.unknowntooltype

## Summary
Deterministic unit coverage for pruning summarizers across known and fallback tool types.

## Evidence
- Source under test: `crates/clankers-agent/src/compaction/tool_summaries.rs`
- Verification rail covers bash, read, write, grep/rg, edit, subagent, and unknown-tool summarizer behavior.

## Checks
- Bash summaries include command, exit status, and output size.
- Read summaries include path, offset, and size.
- Write, grep/rg, and edit summaries include key file or pattern context and size/count info.
- Subagent summaries include delegated goal and result size.
- Unknown tools use generic fallback summarization.
