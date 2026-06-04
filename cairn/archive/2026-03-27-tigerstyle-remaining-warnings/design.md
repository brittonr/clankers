## Context

The clankers workspace has ~272 tigerstyle dylint warnings remaining after two rounds of mechanical fixes (900 → 272). The remaining warnings are structural — they require pattern changes, not string replacement. The fixes span 5 capability areas with distinct refactoring approaches.

The tigerstyle lints run via `cargo dylint` with a custom lint library in `~/git/tigerstyle`. The Nix environment requires a manual driver build (`~/.dylint_drivers/`) and library placement (`~/.cargo-target/dylint/libraries/`).

## Goals / Non-Goals

**Goals:**
- Zero tigerstyle warnings (or documented `#[allow]` with reason for every remaining one)
- Patterns that prevent recurrence (helper functions, macros, exhaustive matches)
- No behavioral changes — all fixes are refactoring

**Non-Goals:**
- Changing public APIs or wire protocols
- Fixing the 2 pre-existing flaky tests (`gc_removes_old_tombstoned`, `test_open_resumes_latest_branch`)
- Modifying external git dependencies (`clanker-actor`, `clanker-router`, etc.)

## Decisions

### D1: Bool struct fields — allow with reason, don't rename
Fields like `bold`, `italic`, `compact`, `focused`, `dirty`, `collapsed`, `read_only` are idiomatic in UI/config code. Renaming to `is_bold` hurts readability at construction sites (`Foo { is_bold: true }` vs `Foo { bold: true }`). Add `#[cfg_attr(dylint_lib = "tigerstyle", allow(bool_naming, reason = "..."))]` on the struct definition.

### D2: Mutex `.expect("not poisoned")` — allow with reason
Poisoned mutex is unrecoverable in this codebase (no cross-thread panic recovery). The `.expect()` is correct behavior. Add allow on the containing function.

### D3: Static regex `.expect("static regex")` — allow with reason
`Regex::new()` on compile-time constant strings inside `LazyLock` can't fail at runtime. The `.expect()` messages document this. Allow on the `LazyLock` static.

### D4: Panel downcast `.expect("registered at startup")` — extract helper
Create `app.panel::<T>(id) -> &T` and `app.panel_mut::<T>(id) -> &mut T` helpers that return `Option` and let callers decide. Most call sites can use `if let Some(panel) = ...` which eliminates the unwrap entirely.

### D5: Event loops — allow with documented reason
`loop { select! { ... } }` and `while let Some(msg) = rx.recv().await` are bounded by the channel being closed. Add `#[allow(unbounded_loop, reason = "...")]` on the containing function.

### D6: Catch-all enum — list all variants
Replace `_ => ...` with explicit variant arms. This is the highest-value fix: the compiler will then flag any new variant additions that need handling.

### D7: Deep nesting — extract helper functions
For each function with nesting > 4, extract the inner logic into a named helper. The helper name documents what the nested block does.

### D8: Acronym style — batch rename
Rename the 5 flagged types from `ALL_CAPS` acronyms to title-case (`RPC` → `Rpc`, `TLS` → `Tls`). Update all callers.

## Risks / Trade-offs

- **Bool renames cascade**: Renaming a struct field touches every construction site and pattern match. Risk of missing a site in a macro or generated code. Mitigated by `cargo check` catching all misses.
- **Enum exhaustive matches bloat**: Some enums have 20+ variants. The `_ =>` existed because most arms do the same thing. Use a local helper or `matches!()` macro to keep the explicit listing compact.
- **Panel helper changes call patterns**: Extracting `app.panel::<T>()` changes the borrow pattern from `app.panels.downcast_ref(id).expect(...)` to `app.panel::<T>(id).unwrap()` or `if let`. This is a mechanical but wide-reaching change.
