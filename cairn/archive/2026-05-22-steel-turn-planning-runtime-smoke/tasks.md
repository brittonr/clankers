# Tasks: Steel Turn Planning Runtime Smoke

- [x] [serial] I1: Add session/controller smoke coverage for config-driven Steel turn planning activation through a real prompt command. [r[steel-turn-planning-runtime-smoke.session-activation]]
- [x] [serial] I2: Add daemon/attach-visible receipt assertions that prove the redacted `steel.host.plan_turn` receipt reaches client-observable events. [r[steel-turn-planning-runtime-smoke.visible-receipt]]
- [x] [parallel] I3: Add fail-closed runtime smokes for invalid script/profile hashes and missing session/UCAN authority. [r[steel-turn-planning-runtime-smoke.fail-closed]]
- [x] [parallel] I4: Add docs plus a Rust checker receipt under `target/steel-turn-planning-runtime-smoke/`. [r[steel-turn-planning-runtime-smoke.receipt-rail]]
- [x] [serial] V1: Run focused Rust tests, checker, Cairn validate/gates, and diff checks before archive. [r[steel-turn-planning-runtime-smoke.verification]]
- [x] [serial] V2: Sync/archive the Cairn and land on clean pushed `main`. [r[steel-turn-planning-runtime-smoke.archive]]
