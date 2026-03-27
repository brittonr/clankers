## 1. Bool naming (77 warnings)

- [ ] 1.1 Audit all 77 `bool_naming` warnings — categorize each as "rename" or "allow" based on whether `is_`/`has_` prefix improves or hurts readability
- [ ] 1.2 Add `#[cfg_attr(dylint_lib = "tigerstyle", allow(bool_naming, reason = "..."))]` to struct definitions with idiomatic field names (`bold`, `italic`, `compact`, `focused`, `dirty`, `collapsed`, `read_only`, `locked`, `allowed`, `enabled`, `expired`, `authenticated`, `subscribed`, `heartbeat`, `matrix`, `mdns`)
- [ ] 1.3 Rename remaining local variables and parameters: apply predicate prefixes, update all callers, verify with `cargo check`
- [ ] 1.4 Verify zero `bool_naming` warnings with `cargo dylint`

## 2. Error propagation — unwrap/expect (72 warnings)

- [ ] 2.1 Add `#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, ...))]` to all `LazyLock<Regex>` statics and mutex acquisition functions with documented reasons
- [ ] 2.2 Extract panel accessor helpers `app.panel::<T>(id) -> Option<&T>` and `app.panel_mut::<T>(id) -> Option<&mut T>` in `crates/clankers-tui/src/app/mod.rs`
- [ ] 2.3 Replace all `app.panels.downcast_ref::<T>(id).expect("registered at startup")` with the new helpers (event_loop_runner, slash_commands/handlers, rpc_embed)
- [ ] 2.4 Replace remaining `.unwrap()` on non-test code: convert to `?` propagation, `.ok()`, or `#[allow]` with reason
- [ ] 2.5 Verify zero `no_unwrap` warnings with `cargo dylint`

## 3. Panic removal (16 warnings)

- [ ] 3.1 Audit all 16 `no_panic` warnings — identify which are `unreachable!()`, `todo!()`, `unimplemented!()`, or explicit `panic!()`
- [ ] 3.2 Replace `todo!()`/`unimplemented!()` with error returns or reasonable defaults
- [ ] 3.3 Replace `panic!()` with error returns; keep `unreachable!()` only in provably unreachable arms with `#[allow]`
- [ ] 3.4 Verify zero `no_panic` warnings with `cargo dylint`

## 4. Unbounded loops (39 warnings)

- [ ] 4.1 Audit all 39 `unbounded_loop` warnings — categorize as event loops (channel/select), retry loops, or iteration loops
- [ ] 4.2 Add `#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "..."))]` to event loop functions (channel receivers, select loops, daemon main loops)
- [ ] 4.3 Add explicit iteration bounds (`.take(MAX)` or counter) to retry/poll loops
- [ ] 4.4 Verify zero `unbounded_loop` warnings with `cargo dylint`

## 5. Catch-all enum matches (17 warnings)

- [ ] 5.1 List all 17 `catch_all_on_enum` sites with the enum type and current default behavior
- [ ] 5.2 Replace each `_ =>` with explicit variant listing using `|` for shared handlers
- [ ] 5.3 Verify exhaustive matches compile and zero `catch_all_on_enum` warnings remain

## 6. Structural cleanup (nesting, length, acronyms, division)

- [ ] 6.1 Fix 11 `nested_conditionals` warnings: extract inner logic into named helper functions
- [ ] 6.2 Fix 8 `function_length` warnings: decompose functions over 100 lines
- [ ] 6.3 Fix 5 `acronym_style` warnings: rename types to title-case acronyms, update all references
- [ ] 6.4 Fix 18 `unchecked_division` warnings: add `#[allow]` with reason for guarded divisions, add `checked_div` for unguarded ones
- [ ] 6.5 Fix 4 remaining `ignored_result` warnings

## 7. Verification

- [ ] 7.1 Run full `cargo dylint --all --no-build -- --workspace` — target zero warnings
- [ ] 7.2 Run `cargo clippy -- -D warnings` — verify no regressions
- [ ] 7.3 Run `cargo nextest run --workspace` — verify all tests pass (excluding known flaky: `gc_removes_old_tombstoned`, `test_open_resumes_latest_branch`)
- [ ] 7.4 Commit with detailed message listing warning counts per category
