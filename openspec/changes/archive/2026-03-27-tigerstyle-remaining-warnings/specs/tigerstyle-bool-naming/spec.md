## ADDED Requirements

### Requirement: Boolean bindings use predicate prefixes
All boolean local variables and function parameters use `is_`, `has_`, `was_`, `should_`, `can_`, `will_`, or `needs_` prefixes.

#### Scenario: Local boolean variable
- **WHEN** a local `let` binding has type `bool`
- **THEN** its name starts with a predicate prefix (`is_`, `has_`, `was_`, `should_`, `can_`, `will_`, `needs_`)

#### Scenario: Function parameter
- **WHEN** a function parameter has type `bool`
- **THEN** its name starts with a predicate prefix

### Requirement: Boolean struct fields allow documented exceptions
Struct fields of type `bool` that are display properties, config flags, or state indicators may retain short names with a documented `#[allow]`.

#### Scenario: Idiomatic UI/config field names
- **WHEN** a struct field is `bold`, `italic`, `collapsed`, `focused`, `dirty`, `compact`, `locked`, `allowed`, `read_only`, `enabled`, or `expired`
- **THEN** it carries `#[cfg_attr(dylint_lib = "tigerstyle", allow(bool_naming, reason = "..."))]` on the struct definition
- **AND** the reason documents why the prefix would hurt readability

#### Scenario: Non-idiomatic struct field
- **WHEN** a struct bool field is not in the idiomatic exception list
- **THEN** it is renamed to use a predicate prefix
- **AND** all construction sites and pattern matches are updated

### Requirement: Zero `bool_naming` warnings
After all changes, `cargo dylint` with tigerstyle produces zero `bool_naming` warnings.

#### Scenario: Clean lint run
- **WHEN** `RUSTUP_TOOLCHAIN=nightly cargo dylint --all --no-build -- --workspace` is run
- **THEN** zero lines match `warning: boolean binding`
