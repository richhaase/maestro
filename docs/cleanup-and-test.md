# Maestro Code Cleanup and Testing Plan

## Overview

This document outlines a plan to refactor the Maestro codebase to be more idiomatic, maintainable, and testable. The goal is to separate concerns, improve code organization, and add comprehensive tests for non-UI components.

## Current State Analysis

### Problems Identified

1. **Monolithic Structure**
   - `main.rs` is ~1758 lines containing everything
   - No module separation
   - All concerns mixed together (UI, business logic, state, utilities)

2. **Mixed Concerns**
   - Business logic intertwined with UI rendering
   - Pure functions mixed with Zellij API calls
   - Hard to test pure logic in isolation

3. **Poor Encapsulation**
   - Model has 27 public fields (should use getters/setters or builder pattern)
   - No clear boundaries between components
   - Utility functions scattered throughout

4. **Error Handling**
   - Uses `String` for errors instead of proper error types
   - No error context or structured error information
   - Inconsistent error handling patterns

5. **Testability Issues**
   - No way to test pure business logic in isolation
   - File I/O hardcoded (hard to mock for testing)
   - Pure functions mixed with Zellij API calls

6. **Code Organization**
   - Related functionality not grouped
   - No clear module boundaries
   - Utility functions mixed with business logic

## Proposed Architecture

### Module Structure

```
src/
├── main.rs                 # Plugin entry point, ZellijPlugin trait impl
├── lib.rs                  # Library root (for testing, re-exports)
├── model.rs                # Core state management (Model struct, state transitions)
├── agent.rs                # Agent domain (Agent, AgentPane structs, validation, KDL persistence)
├── ui.rs                   # All UI rendering (render functions, Mode/Section enums)
├── handlers.rs             # Event and key handling (event processing, key handlers)
└── utils.rs                # Pure utility functions (text, workspace, title parsing)
```

**Total: 7 files** (including lib.rs for testing)

This keeps related functionality together while maintaining clear separation of concerns:
- **model.rs**: State and state management (~400-500 lines)
- **agent.rs**: Agent domain model, validation, and persistence (~300-400 lines)
- **ui.rs**: All rendering and UI-related enums (~300-400 lines)
- **handlers.rs**: Event and key handling logic (~200-300 lines)
- **utils.rs**: Pure utility functions (~100-200 lines)
- **main.rs**: Plugin integration (~100-150 lines)

### Separation of Concerns

1. **Domain Models** (`agent.rs`)
   - Agent and AgentPane structs
   - Validation logic
   - KDL serialization/deserialization
   - Agent persistence (file I/O)
   - **Highly testable**

2. **State Management** (`model.rs`)
   - Model struct with state
   - State transitions
   - Selection and filtering logic
   - **Testable for state logic**

3. **Event Handlers** (`handlers.rs`)
   - Event processing logic
   - Key event handling
   - State updates
   - Direct Zellij API calls (expected for a plugin)
   - **Testable for pure logic, accept Zellij calls**

4. **UI Rendering** (`ui.rs`)
   - All rendering functions
   - Mode and Section enums
   - Form field navigation
   - **Not tested** (WASM/UI specific)

5. **Utilities** (`utils.rs`)
   - Pure utility functions (text, workspace, title parsing)
   - **Highly testable**

6. **Plugin Integration** (`main.rs`)
   - ZellijPlugin trait implementation
   - Event routing
   - **Not tested** (WASM specific)

## Refactoring Steps

### Phase 1: Extract Domain Models and Utilities (Low Risk)

**Goal**: Extract pure, testable code without changing behavior.

1. **Create `src/agent.rs`**
   - Move `Agent` struct
   - Move `AgentPane` struct
   - Move `PaneStatus` enum
   - Move KDL serialization from `agents.rs`
   - Add validation methods
   - Add tests for validation and serialization

2. **Create `src/utils.rs`**
   - Move `truncate()` function
   - Move `truncate_path()` function
   - Move `workspace_basename()` function
   - Move `workspace_tab_name()` function
   - Move `is_maestro_tab()` function
   - Move `parse_title_hint()` function
   - Move `build_command_with_env()` function
   - Move `parse_env_input()` function
   - Add tests for all utilities

3. **Create `src/ui.rs`**
   - Move `Mode` enum
   - Move `AgentFormField` enum
   - Move `Section` enum
   - Move `next_field()` and `prev_field()` functions
   - Move all `render_*` functions
   - Add tests for field navigation and enum logic

**Testing**: Add unit tests for all extracted functions.

### Phase 2: Refactor Agent Persistence (Low-Medium Risk)

**Goal**: Make agent persistence testable.

1. **In `src/agent.rs`**
   - Refactor KDL serialization functions to be pure
   - Make config path configurable (for testing)
   - Add functions: `load_agents(path: &Path)`, `save_agents(path: &Path, agents: &[Agent])`
   - Keep default path helper for production use
   - Add tests with temp files

**Note**: We keep Zellij API calls directly in handlers - no need to abstract them since this is a Zellij plugin.

**Testing**: Add unit tests with temp files for persistence.

### Phase 3: Refactor Model (Medium-High Risk)

**Goal**: Improve encapsulation and separate concerns.

1. **Refactor `src/model.rs`**
   - Make fields private
   - Add getters/setters where needed
   - Extract form state into separate struct (internal)
   - Extract selection state into separate struct (internal)
   - Keep all state management logic together

2. **Extract business logic from Model**:
   - Move agent persistence calls to use functions from `agent.rs`
   - Keep pane operations that call Zellij API directly (expected)
   - Keep state management, selection, and filtering in Model

**Testing**: Add integration tests for state transitions.

### Phase 4: Extract Event Handlers (Medium Risk)

**Goal**: Separate event handling from state management.

1. **Create `src/handlers.rs`**
   - Extract all `handle_*` methods from Model
   - Extract all `handle_key_event_*` methods
   - Extract pane lifecycle management logic
   - Keep direct Zellij API calls (this is a plugin)
   - Test pure logic parts (state updates, parsing)

**Testing**: Add unit tests for pure logic in handlers (state updates, parsing).

### Phase 5: Extract UI Rendering (Low Risk)

**Goal**: Separate all rendering code.

1. **In `src/ui.rs`** (already created in Phase 1)
   - All `render_*` functions are already here
   - Keep Zellij UI component dependencies
   - **No tests** (UI specific)

2. **Update `src/main.rs`**
   - Keep only plugin registration and ZellijPlugin trait impl
   - Delegate to handlers and model
   - Minimal code (~100-150 lines)

### Phase 6: Error Handling (Low Risk)

**Goal**: Replace String errors with proper error types.

1. **Create `src/error.rs`**
   ```rust
   #[derive(Debug, thiserror::Error)]
   pub enum MaestroError {
       #[error("Agent error: {0}")]
       Agent(#[from] AgentError),
       #[error("Config error: {0}")]
       Config(#[from] ConfigError),
       // ... etc
   }
   ```

2. **Add `thiserror` dependency** to Cargo.toml

3. **Replace all `String` errors** with proper error types

**Testing**: Add error handling tests.

### Phase 7: Add Comprehensive Tests

**Goal**: Achieve high test coverage for non-UI code.

1. **Unit Tests** (in `src/*/tests.rs` or `tests/` directory)
   - Domain model validation (`agent.rs`)
   - Utility functions (`utils.rs`)
   - Agent persistence (with temp files in `agent.rs`)
   - Pure logic in handlers (state updates, parsing in `handlers.rs`)

2. **Integration Tests** (in `tests/`)
   - Agent persistence (with temp files)
   - State transitions
   - Form validation

3. **Test Utilities**
   - Mock implementations of traits
   - Test fixtures
   - Helper functions

## Testing Strategy

### What to Test

✅ **Test These**:
- Agent validation and business rules
- KDL serialization/deserialization
- Title parsing (`is_maestro_tab`, `parse_title_hint`)
- Workspace name generation
- Command building
- Form field navigation
- Text manipulation utilities
- State transitions (modes, sections)
- Filtering logic
- Error handling

❌ **Don't Test These**:
- ZellijPlugin trait implementation
- Direct Zellij API calls (accept them as part of plugin)
- Rendering functions (depend on Zellij UI components)
- Integration with Zellij runtime

### Test Organization

```
tests/
├── agent.rs              # Agent validation, persistence tests
├── utils.rs              # Utility function tests
├── model.rs              # State transition tests
└── helpers.rs            # Test helpers and fixtures
```

Or use inline tests in `src/*/tests.rs` modules:
- `src/agent.rs` → `#[cfg(test)] mod tests { ... }`
- `src/utils.rs` → `#[cfg(test)] mod tests { ... }`
- `src/model.rs` → `#[cfg(test)] mod tests { ... }`

## Rust Idioms to Apply

1. **Error Handling**
   - Use `Result<T, E>` with proper error types
   - Use `thiserror` or `anyhow` for error types
   - Avoid `String` errors

2. **Encapsulation**
   - Make fields private
   - Use getters/setters or builder patterns
   - Expose only necessary API

3. **Type Safety**
   - Use newtype patterns where appropriate
   - Avoid primitive obsession
   - Use enums for state machines

4. **Testability**
   - Extract pure functions for testing
   - Use configurable paths for file I/O (testable)
   - Accept Zellij API calls as part of plugin (don't abstract)
   - Avoid global state

5. **Module Organization**
   - One module per concern
   - Clear module boundaries
   - Public API at module level

6. **Documentation**
   - Add doc comments to public API
   - Document error conditions
   - Add examples where helpful

## Implementation Order

1. **Start with Phase 1** (utilities and domain models)
   - Lowest risk
   - Immediate testability gains
   - No behavior changes

2. **Then Phase 6** (error handling)
   - Can be done incrementally
   - Improves code quality

3. **Then Phase 2** (agent service)
   - Enables Phase 3
   - Creates testability foundation for persistence

4. **Then Phase 3** (model refactoring)
   - Depends on services
   - Most complex refactoring

5. **Then Phase 4** (handlers)
   - Depends on model refactoring
   - Separates concerns

6. **Finally Phase 5** (UI extraction)
   - Low risk
   - Cleanup step

## Success Criteria

- [ ] `main.rs` under 150 lines
- [ ] All business logic in testable modules
- [ ] Test coverage > 80% for non-UI code
- [ ] No public fields in Model (encapsulated)
- [ ] All errors use proper error types
- [ ] Clear module boundaries
- [ ] All tests pass
- [ ] Plugin still works in Zellij
- [ ] Code compiles without warnings
- [ ] Clippy passes with no warnings

## Risk Mitigation

1. **Incremental Refactoring**
   - One module at a time
   - Test after each change
   - Keep plugin working throughout

2. **Feature Flags** (if needed)
   - Can enable/disable refactored code
   - Fallback to old code if issues

3. **Comprehensive Testing**
   - Test each phase before moving on
   - Integration tests catch regressions

4. **Version Control**
   - Commit after each successful phase
   - Easy to rollback if needed

## Dependencies to Add

```toml
[dev-dependencies]
tempfile = "3"  # Already present
assert_matches = "1.5"  # For pattern matching in tests

[dependencies]
thiserror = "1.0"  # For error types
```

**Note**: Removed `mockall` - we'll use temp files for testing file I/O instead of mocking.

## Notes

- **This is a Zellij plugin** - direct Zellij API calls are expected and appropriate
- Keep WASM-specific code isolated
- Don't break existing functionality
- Test incrementally
- Focus on extracting pure functions and business logic for testing
- Accept that some code will call Zellij API directly (that's the point of a plugin)
- Document public API
- Follow Rust naming conventions
- Use `cargo clippy` and `cargo fmt`
