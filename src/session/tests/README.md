# Session Tests Organization

This directory contains the test suite for the session module, split from the original monolithic `tests.rs` (1020 lines) into focused modules.

## Structure

- **mod.rs** (460 bytes) - Module setup with common imports and submodule declarations
- **store_tests.rs** (3.7 KB, 6 tests) - Session creation, opening, listing, and persistence tests
- **context.rs** (2.3 KB, 2 tests) - Context building and session resume tests
- **tree.rs** (8.5 KB, 7 tests) - Tree/branch management and tracking tests
- **navigation.rs** (6.2 KB, 9 tests) - Navigation operations (set_active_head, rewind, resolve_target)
- **labels.rs** (1.3 KB, 2 tests) - Label recording and validation tests
- **merge.rs** (13 KB, 12 tests) - Merge operations (full merge, selective merge, cherry-pick)

## Total

- **7 files**
- **1,076 lines** (including module overhead)
- **38 tests** covering all session functionality

## Benefits

1. **Easier navigation** - Find tests by topic rather than scrolling through 1000+ lines
2. **Focused changes** - Modify merge tests without touching store tests
3. **Better organization** - Clear separation of concerns matches the module structure
4. **Faster compilation** - Rust can parallelize compilation of independent modules
5. **Clearer intent** - Module names indicate what functionality is being tested
