## ADDED Requirements

### Requirement: No unhandled `.unwrap()` or `.expect()` in production code
All `.unwrap()` and `.expect()` calls in non-test code either propagate errors with `?`, use `.ok()` for intentional discard, or carry a documented `#[allow]`.

#### Scenario: Mutex/RwLock acquisition
- **WHEN** code calls `.lock().expect(...)` or `.write().expect(...)` on a Mutex/RwLock
- **THEN** the containing function has `#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "mutex poisoning is unrecoverable"))]`

#### Scenario: Static regex compilation
- **WHEN** `Regex::new()` is called on a compile-time constant string inside a `LazyLock` or `static`
- **THEN** `.expect("static regex")` is permitted
- **AND** the `LazyLock` static has `#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "compile-time constant pattern"))]`

#### Scenario: Panel downcast in TUI
- **WHEN** `app.panels.downcast_ref::<T>(id).expect(...)` is used to access a panel registered at startup
- **THEN** it is replaced by a helper that returns `Option<&T>`
- **AND** the call site uses `if let Some(panel) = ...` or early return

#### Scenario: Protocol frame `.expect("u32 fits in usize")`
- **WHEN** `usize::try_from(u32_value).expect(...)` converts a protocol length
- **THEN** it uses `.unwrap_or(0)` with a compile-time assertion `const _: () = assert!(u32::MAX as u128 <= usize::MAX as u128)`

### Requirement: No `panic!()` in production code
All `panic!()`, `todo!()`, and `unimplemented!()` macros in non-test code are replaced with error returns or `debug_assert!`.

#### Scenario: Unreachable match arms
- **WHEN** a match arm uses `unreachable!()` in a provably unreachable position (e.g., after exhaustive guards)
- **THEN** it is replaced with `debug_unreachable!()` or `#[cfg_attr(dylint_lib = "tigerstyle", allow(no_panic, reason = "..."))]`

#### Scenario: Todo markers
- **WHEN** `todo!()` or `unimplemented!()` exists in production code
- **THEN** it is replaced with an error return or a reasonable default

### Requirement: Zero `no_unwrap` and `no_panic` warnings
After all changes, `cargo dylint` with tigerstyle produces zero `no_unwrap` and `no_panic` warnings.

#### Scenario: Clean lint run
- **WHEN** the tigerstyle lints are run
- **THEN** zero lines match `warning:.*in production code will panic` or `warning:.*will abort the process`
