## ADDED Requirements

### Requirement: All loops have visible iteration bounds
Every `loop`, `while`, and `for` construct either has a visible bound (counter, `.take()`, max iterations) or a documented `#[allow]`.

#### Scenario: Event loop on channel receiver
- **WHEN** a loop reads from a channel with `while let Some(msg) = rx.recv().await`
- **THEN** the containing function has `#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "bounded by channel close"))]`

#### Scenario: `select!` event loop
- **WHEN** `loop { tokio::select! { ... } }` processes events until a shutdown signal
- **THEN** the containing function has `#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; exits on shutdown signal"))]`

#### Scenario: Retry/poll loop
- **WHEN** a loop retries an operation until success
- **THEN** it uses `.take(MAX_RETRIES)` or a counter with `if attempts >= MAX { break; }`

#### Scenario: Iterator consumption loop
- **WHEN** a `for` loop iterates over a collection
- **THEN** the lint does not fire (iterators are inherently bounded)

### Requirement: Zero `unbounded_loop` warnings
After all changes, `cargo dylint` with tigerstyle produces zero `unbounded_loop` warnings.

#### Scenario: Clean lint run
- **WHEN** the tigerstyle lints are run
- **THEN** zero lines match `warning: loop without visible iteration bound`
