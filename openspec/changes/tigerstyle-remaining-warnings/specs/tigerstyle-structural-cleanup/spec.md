## ADDED Requirements

### Requirement: Conditional nesting ≤ 4 levels
No function has conditional nesting deeper than 4 levels.

#### Scenario: Deeply nested conditional
- **WHEN** a function has `if`/`match`/`while` nesting at 5+ levels
- **THEN** the inner logic is extracted into a named helper function
- **AND** the helper name documents what the nested block does

### Requirement: Functions ≤ 100 lines
No function body exceeds 100 lines (excluding blank lines and comments).

#### Scenario: Long function
- **WHEN** a function body exceeds 100 lines
- **THEN** it is decomposed into smaller functions with clear responsibilities
- **AND** the parent function reads as a sequence of high-level steps

### Requirement: Acronym-style type names
Type names use title-case for acronyms: `Rpc`, `Tls`, `Sql` — not `RPC`, `TLS`, `SQL`.

#### Scenario: Acronym in type name
- **WHEN** a struct, enum, or trait name contains an all-caps acronym of 3+ letters
- **THEN** it is renamed to title-case (e.g., `RPCHandler` → `RpcHandler`)
- **AND** all references are updated

### Requirement: Divisions checked for zero
All integer division and modulo operations are guarded against zero divisors.

#### Scenario: Division guarded by is_empty check
- **WHEN** `x % vec.len()` or `x / vec.len()` appears after an `if vec.is_empty() { return; }` guard
- **THEN** it carries `#[cfg_attr(dylint_lib = "tigerstyle", allow(unchecked_division, reason = "guarded by is_empty check above"))]`

#### Scenario: Division with const-size arrays
- **WHEN** division is by `CONST_ARRAY.len()` where the array is a non-empty constant
- **THEN** it carries `#[cfg_attr(dylint_lib = "tigerstyle", allow(unchecked_division, reason = "const non-empty array"))]`

### Requirement: Zero structural warnings
After all changes, `cargo dylint` produces zero warnings for `nested_conditionals`, `function_length`, `acronym_style`, `unchecked_division`.

#### Scenario: Clean lint run
- **WHEN** the tigerstyle lints are run
- **THEN** zero lines match `warning: conditional nesting`, `warning: function.*is.*lines`, `warning:.*acronym-style`, or `warning: division by`
