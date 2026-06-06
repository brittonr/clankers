# Change: Extract Process Job Contracts

## Why

Process-job policy is still split across the root `process` tool, runtime facade DTOs, and backend adapters. That makes reusable admission/profile/receipt behavior harder to test without shell code and keeps root modules responsible for policy that should be owned by a neutral service boundary.

## What Changes

- Extract process-job admission, profile, notification, retention, redaction, and receipt DTOs into a green process-job contract crate or similarly neutral owner.
- Keep native, pueue, systemd, procmon, filesystem, and TUI projection behavior in shell/backend adapters.
- Update architecture and runtime-facade inventories so the root process tool is proven to be JSON parsing, typed-service dispatch, and receipt projection only.

## Impact

- **Files**: `crates/clankers-runtime/src/process_jobs.rs`, `src/tools/process*.rs`, process backend modules, generated runtime facade inventory, and process/job architecture rails.
- **Testing**: focused process-job policy tests, process backend capability/security/redaction rails, runtime facade boundary rail, FCIS boundary rail, and aggregate embedded SDK acceptance if public SDK labels move.
