## ADDED Requirements

### Requirement: No catch-all `_ =>` on enum matches
All `match` expressions on enum types list variants explicitly instead of using `_ =>`.

#### Scenario: Enum with shared default handling
- **WHEN** multiple variants share the same handler
- **THEN** they are listed explicitly with `|` (e.g., `Variant::A | Variant::B | Variant::C => default_handler()`)

#### Scenario: Large enum with grouped handling
- **WHEN** an enum has 10+ variants where most share a handler
- **THEN** a `matches!(value, Variant::A | Variant::B | ...)` guard or helper function groups them
- **AND** the match still lists all variants

#### Scenario: New variant added to enum
- **WHEN** a new variant is added to an enum used in an exhaustive match
- **THEN** the compiler emits a `non-exhaustive patterns` error at every match site
- **AND** the developer must decide how to handle the new variant

### Requirement: Zero `catch_all_on_enum` warnings
After all changes, `cargo dylint` with tigerstyle produces zero catch-all warnings.

#### Scenario: Clean lint run
- **WHEN** the tigerstyle lints are run
- **THEN** zero lines match `warning: catch-all.*on enum match`
