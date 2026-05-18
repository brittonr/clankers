## Phase 1: Event model

- [x] [serial] [covers=process-job-notification-events.schema.common] Define typed completion/readiness event DTOs with event ids, job ids, owner scope, backend, safe excerpts, and log refs.
- [x] [parallel] [covers=process-job-notification-events.delivery.sink-interface] Define `ProcessJobNotificationSink` and fake sink contracts decoupled from backend implementations.
- [x] [parallel] [covers=process-job-notification-events.rate-limit.watch-patterns] Define readiness rate-limit/suppression state and deterministic policy tests.

## Phase 2: Delivery and replay

- [x] [serial] [depends:phase-1] Add service-layer notification policy that consumes backend facts and writes persisted notification events.
- [x] [parallel] [covers=process-job-notification-events.delivery.multi-client-dedup] Implement event id/dedup handling for multiple attached clients.
- [x] [parallel] [covers=process-job-notification-events.replay.reattach] Implement authorization-filtered replay for detached/reattached sessions.

## Phase 3: Verification

- [x] [serial] [depends:phase-2] Add fake sink tests, detach/reattach replay tests, and unauthorized replay denial tests.
- [x] [serial] [depends:phase-2] Add noisy pattern suppression tests and exactly-once completion tests.
- [x] [serial] [depends:phase-2] Run focused notification tests, `openspec validate define-process-job-notification-events --strict --json`, and `git diff --check`.
