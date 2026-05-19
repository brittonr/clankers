# Durable Process Jobs

Clankers exposes durable background work through the agent `process` tool. Use it for long-running builds, test suites, servers, and watchers instead of shell-level `&`, `nohup`, or `disown`.

## Backends

The `process` tool accepts a `backend` field on `start`, `list`, `poll`, `log`, `wait`, `kill`, `restart`, and `adopt` actions:

| Backend | Use when | Notes |
| --- | --- | --- |
| `native` | You want the built-in local process registry. | Default backend. Tracks child processes, incremental logs, stdin, completion state, and daemon restart reconciliation. |
| `pueue` | You want queueing/group concurrency through pueue. | Set `group` and `label` on starts when useful. Existing pueue tasks can be adopted with `pueue_task_id` or `backend_ref: "pueue:<id>"`. |
| `systemd` | You want transient systemd units and host-level resource controls. | Existing units can be adopted with `systemd_unit` or `backend_ref: "systemd:<unit>"`. NixOS deployments can configure `unitPrefix`, resource limits, working directory, writable paths, and kill grace. |

Example tool payloads:

```json
{ "action": "start", "backend": "native", "command": "cargo nextest run", "notify_on_complete": true }
```

```json
{ "action": "start", "backend": "pueue", "group": "clankers", "label": "slow-check", "command": "nix flake check" }
```

```json
{ "action": "start", "backend": "systemd", "label": "daemon-smoke", "program": "bash", "args": ["-lc", "./scripts/verify.sh"] }
```

## Receipts and availability errors

Every backend operation returns a typed receipt shape with `operation`, `id`, `backend`, `status`, `backend_ref`, `log_refs`, and a bounded human summary when those fields apply. Unsupported or unavailable backends fail explicitly instead of silently falling back: for example, a missing pueue binary/daemon or unavailable systemd runner is reported as a backend availability error, while invalid profile/backend policy is reported before backend dispatch.

## Notifications

`notify_on_complete: true` emits one notification when the job reaches a terminal state. Use this for builds, test suites, deploys, and other finite jobs.

`watch_patterns` is for rare readiness signals from long-lived jobs, such as `Application startup complete`. Matches are rate-limited and repeated noisy matches are suppressed. Do not use `watch_patterns` for end-of-run markers on finite jobs; prefer `notify_on_complete`.

## Retention and garbage collection

Completed process/job records and retained logs are garbage-collected by policy. The `gc`/`garbage_collect` action accepts optional overrides:

```json
{ "action": "gc", "max_age_days": 14, "max_records": 1000, "max_log_bytes": 1073741824 }
```

List operations also apply the default retention policy before projecting results, so expired terminal records do not remain visible indefinitely.

## NixOS service configuration

The NixOS module enables durable process management under `services.clankers-daemon.processManagement`:

```nix
{
  services.clankers-daemon = {
    enable = true;
    processManagement = {
      enable = true;
      defaultBackend = "native";
      retention = {
        maxAgeDays = 14;
        maxRecords = 1000;
        maxLogBytes = 1073741824;
      };
      pueue = {
        enable = true;
        groups.clankers = 4;
      };
      systemd = {
        enable = true;
        unitPrefix = "clankers-job";
        runtimeMaxSec = 3600;
      };
    };
  };
}
```

The module creates the process-job registry and log directories below the daemon `stateDir` by default and can manage a dedicated pueue service when the pueue backend is enabled.

## Project job profiles

The `process-job-profile-kit` is the copyable brick for backend-neutral process-job manifests. Project-defined profiles parse into backend-neutral `StartProcessJobRequest` values before any backend dispatch. A profile must set exactly one of `command` or `program`; `args` is valid only with `program`. Policy controls the default backend, allowed backends, maximum timeout/memory/CPU bounds, and allowed environment-variable prefixes. Secret-like environment keys such as `APP_TOKEN`, `APP_SECRET`, or `APP_KEY` fail closed before backend dispatch.

Reusable behavior lives in `ProjectProcessJobProfiles`, `ProjectProcessJobProfilePolicy`, `StartProcessJobRequest`, `ProcessJobIdentityEnvelope`, and `ProcessJobRedactionPolicy`. Product-owned behavior remains outside the brick: selecting a daemon/session, spawning native/pueue/systemd jobs, persisting receipts/logs, and notifying users. Resolving a profile is pure: it validates policy and returns a start request, but does not spawn a process, contact pueue/systemd, or write storage.

Profile JSON shape:

```json
{
  "profiles": {
    "quick-check": {
      "backend": "native",
      "command": "cargo check --tests",
      "cwd": "/home/example/project",
      "notification_policy": { "notify_on_complete": true },
      "metadata": { "purpose": "developer-smoke" }
    }
  }
}
```

Profiles are pure configuration: resolving them validates policy and produces a start request; it does not spawn a process, contact pueue/systemd, or write storage by itself. The focused drift rail is `scripts/check-process-job-profile-kit.rs`.
