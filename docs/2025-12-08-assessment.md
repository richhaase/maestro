# Maestro Code Assessment

**Date**: 2025-12-08
**Reviewer**: Claude (Opus 4.5)
**Codebase Version**: 0.1.4 (commit 18ce3c3)

---

## Executive Summary

The codebase is well-structured for a Zellij plugin with good separation of concerns across modules (`model.rs`, `handlers.rs`, `ui.rs`, `agent.rs`, `utils.rs`). The code works correctly and has reasonable test coverage for core functionality. However, there are notable inconsistencies in patterns, some questionable design decisions, and UX gaps that prevent it from being great.

**Lines of Code**: ~2,933 across 7 source files

---

## 1. Architectural & Design Pattern Issues

### 1.1 Verbose Accessor Pattern in Model

The `Model` struct uses an accessor pattern with both `foo()` and `foo_mut()` methods for every field, resulting in 50+ boilerplate methods (lines 41-236 in `model.rs`).

```rust
// model.rs:42-48 - Some return slices
pub fn agents(&self) -> &[Agent] { &self.agents }
pub fn agents_mut(&mut self) -> &mut Vec<Agent> { &mut self.agents }

// model.rs:66-68 - Some return references
pub fn error_message(&self) -> &str { &self.error_message }

// model.rs:78-79 - Some return Copy types
pub fn mode(&self) -> Mode { self.mode }

// model.rs:206-208 - Some return Option<&T>
pub fn session_name(&self) -> Option<&String> { self.session_name.as_ref() }
```

**Problem**: This creates significant maintenance burden without clear benefit. The `Model` struct is only accessed from `handlers.rs` and `ui.rs`—both internal modules.

**Recommendation**: Either make fields `pub(crate)` for direct access, or use a more focused pattern (e.g., separate `FormState` struct with its own methods).

### 1.2 Dual Validation Systems

Validation logic is duplicated between `Agent::validate()` and `build_agent_from_inputs()`:

```rust
// agent.rs:37-46 - Uses anyhow
impl Agent {
    pub fn validate(&self) -> Result<()> {
        if name.is_empty() { bail!("agent name is required"); }
        if self.command.trim().is_empty() { bail!("agent command is required"); }
    }
}

// handlers.rs:483-514 - Uses MaestroError
fn build_agent_from_inputs(model: &Model) -> MaestroResult<Agent> {
    if name.is_empty() { return Err(MaestroError::AgentNameRequired); }
    if command.is_empty() { return Err(MaestroError::CommandRequired); }
}
```

**Problem**: Two different error types for the same validation. `Agent::validate()` is defined but never called anywhere in the codebase.

**Recommendation**: Remove `Agent::validate()` or consolidate validation into a single location using `MaestroError`.

### 1.3 ~~Unused Code~~ ✅ RESOLVED

~~| Item | Location | Issue |~~
~~|------|----------|-------|~~
~~| `MaestroError::EnvParse` | `error.rs:22` | Defined but never constructed |~~
~~| `Agent::validate()` | `agent.rs:37-46` | Never called |~~
~~| `save_agents_default()` | `agent.rs:139-150` | Duplicates logic in `handlers.rs:544-566` |~~
~~| `truncate_path()` | `utils.rs:137-170` | Defined but never used |~~

*All unused code removed (~125 lines) on 2025-12-08.*

---

## 2. Error Handling Inconsistencies

### 2.1 Mixed Error Types

The codebase uses three different error handling approaches:

| Module | Error Type | Example |
|--------|------------|---------|
| `error.rs` | `MaestroError` (thiserror) | Form validation |
| `agent.rs` | `anyhow::Result` | File I/O |
| `utils.rs` | `Result<T, String>` | Directory reading |

```rust
// utils.rs:14
pub fn read_directory(path: &Path) -> Result<Vec<DirEntry>, String>

// agent.rs:49
pub fn load_agents(path: &Path) -> Result<Vec<Agent>>  // anyhow

// handlers.rs:483
fn build_agent_from_inputs(model: &Model) -> MaestroResult<Agent>
```

**Recommendation**: Standardize on `MaestroError` for all errors that may surface to users, use `anyhow` internally for context chaining.

### 2.2 Direct Error Message Setting

Some errors bypass the type system entirely:

```rust
// handlers.rs:367 - Direct string
*model.error_message_mut() = "permissions not granted".to_string();

// handlers.rs:759 - Through error type
*model.error_message_mut() = err.to_string();
```

**Recommendation**: All user-facing errors should go through `MaestroError` for consistency and testability.

---

## 3. UX Issues

### 3.1 Inconsistent Key Bindings

| Mode | Up | Down | Back |
|------|-----|------|------|
| View | `k` / `Up` | `j` / `Down` | `Esc` |
| AgentConfig | `k` / `Up` | `j` / `Down` | `Esc` |
| **NewPaneWorkspace** | `Up` only | `Down` only | `Esc` |
| NewPaneAgentSelect | `k` / `Up` | `j` / `Down` | `Esc` |

**Problem**: `NewPaneWorkspace` mode (`handlers.rs:647-690`) doesn't support `j`/`k` navigation while all other modes do. This breaks muscle memory and feels inconsistent.

**Recommendation**: Add `j`/`k` bindings to `NewPaneWorkspace` mode.

### 3.2 Limited Text Input

```rust
// handlers.rs:40-56
fn handle_text_edit(target: &mut String, key: &KeyWithModifier) -> bool {
    match key.bare_key {
        BareKey::Backspace => { target.pop(); true }
        BareKey::Delete => { target.clear(); true }  // Clears ENTIRE string
        BareKey::Char(c) => { target.push(c); true }
        _ => false,
    }
}
```

**Problems**:
- `Delete` clears the entire input (non-standard behavior)
- No cursor position tracking—can only type at end
- No `Home`/`End`/arrow key support
- No `Ctrl+U` to clear line (Unix convention)

### 3.3 Missing User Feedback

- No confirmation message when agent is created/edited successfully
- No indication of which specific field is invalid on form errors
- Errors persist in status bar until next action (no timeout/clear)

### 3.4 Hidden Tab Name Customization

The `custom_tab_name` field exists in `Model` but isn't exposed in the wizard flow. Tab names are auto-derived from workspace path with no user override option.

---

## 4. Code Organization

### 4.1 Handler Bloat

`handlers.rs` is 1,162 lines with too many responsibilities:

- Key event routing
- Mode-specific handlers (7 modes)
- Pane lifecycle management
- Session/tab state synchronization
- Agent CRUD operations
- Form state management
- Selection movement
- Validation logic

**Recommendation**: Split into focused modules:
- `events.rs` - Event routing and pane lifecycle
- `forms.rs` - Form handling and validation
- `navigation.rs` - Selection and mode transitions

### 4.2 Public API Surface

`lib.rs` exports all modules as public:

```rust
pub mod agent; pub mod error; pub mod handlers; pub mod model; pub mod ui; pub mod utils;
```

For a WASM plugin with a single entry point (`main.rs`), none of these need to be public. All modules could be `pub(crate)`.

---

## 5. Specific Code Issues

### 5.1 Defensive Cloning

```rust
// handlers.rs:82-83 - Double clone
let tab_names: Vec<String> = tabs.iter().map(|t| t.name.clone()).collect();
*model.tab_names_mut() = tab_names.clone();

// handlers.rs:370-371 - Clone entire agent to use a few fields
let agent = match model.agents().iter().find(|a| a.name == agent_name) {
    Some(a) => a.clone(),
```

### 5.2 Magic Numbers

```rust
// ui.rs:88-91 - Undocumented color codes
let status_color = match pane.status {
    PaneStatus::Running => 2,   // Green?
    PaneStatus::Exited(_) => 1, // Red?
};

// ui.rs:184 - Arbitrary limit
let max_display = 5;
```

**Recommendation**: Define named constants:
```rust
const COLOR_SUCCESS: u8 = 2;
const COLOR_ERROR: u8 = 1;
const MAX_SUGGESTIONS_DISPLAYED: usize = 5;
```

### 5.3 ~~Hardcoded Paths~~ ✅ RESOLVED

~~The `/host` WASI mount path appears in multiple locations:~~

~~- `agent.rs:253` - `config_base_dir()`~~
~~- `utils.rs:52-54` - `get_path_suggestions()`~~
~~- `utils.rs:109-112` - Path formatting~~
~~- `utils.rs:202` - `get_home_directory()` fallback~~
~~- `utils.rs:214-225` - `resolve_workspace_path()`~~
~~- `ui.rs:173, 201` - Display stripping~~

*Added `WASI_HOST_MOUNT` constant in `lib.rs`, updated all 15 references on 2025-12-08.*

### 5.4 Unused Variable

```rust
// handlers.rs:395
let (tab_target, _is_new_tab) = match tab_choice {
```

The `_is_new_tab` is computed but never used.

### 5.5 Redundant Import

```rust
// handlers.rs:7-8
use zellij_tile::prelude::*;
use zellij_tile::prelude::{
    BareKey, KeyModifier, KeyWithModifier, PaneId, PaneManifest, PermissionStatus, TabInfo,
};
```

The glob import already includes these items.

---

## 6. Testing Gaps

### 6.1 No Integration Tests

All tests are unit tests. Missing coverage for:
- Full key event flows through mode transitions
- Tab/pane lifecycle end-to-end scenarios
- Config file round-tripping with actual filesystem

### 6.2 ~~Duplicated Test Helpers~~ ✅ RESOLVED

~~`create_test_agent()` is defined identically in both:~~
~~- `model.rs:244-251`~~
~~- `handlers.rs:920-927`~~

*Extracted to `test_helpers` module in `lib.rs` on 2025-12-08.*

### 6.3 Untested Critical Paths

| Function | Location | Risk |
|----------|----------|------|
| `render_ui()` | `ui.rs:59` | No tests for any rendering |
| `handle_session_update()` | `handlers.rs:334` | Complex state management |
| `rebuild_from_session_infos()` | `handlers.rs:210` | 85 lines of complex logic |
| `handle_key_event_delete_confirm()` | `handlers.rs:775` | Destructive operation |
| `get_path_suggestions()` | `utils.rs:41` | Edge cases untested |

---

## 7. Documentation

- No doc comments on public types or functions
- Complex algorithms (e.g., `rebuild_from_session_infos`) have no inline documentation
- `CLAUDE.md` serves as external documentation but isn't linked to code via `#![doc]`

---

## 8. Summary of Recommendations

### High Priority

| Issue | Action | Impact | Status |
|-------|--------|--------|--------|
| Inconsistent `j`/`k` bindings | Add to `NewPaneWorkspace` mode | UX consistency | |
| Mixed error handling | Standardize on `MaestroError` | Maintainability | |
| Handler bloat | Split `handlers.rs` into modules | Code organization | |
| ~~Dead code~~ | ~~Remove unused items~~ | ~~Clarity~~ | ✅ Done |

### Medium Priority

| Issue | Action | Impact | Status |
|-------|--------|--------|--------|
| Magic numbers | Extract to named constants | Readability | |
| ~~Hardcoded `/host`~~ | ~~Define constant~~ | ~~Maintainability~~ | ✅ Done |
| ~~Test helpers~~ | ~~Extract to shared module~~ | ~~Test maintainability~~ | ✅ Done |
| Documentation | Add doc comments to public API | Onboarding | |

### Low Priority

| Issue | Action | Impact |
|-------|--------|--------|
| Accessor boilerplate | Make fields `pub(crate)` | Code reduction |
| Defensive cloning | Use references where possible | Performance |
| Public API surface | Change modules to `pub(crate)` | Encapsulation |

---

## Appendix: File Statistics

| File | Lines | Responsibility |
|------|-------|----------------|
| `handlers.rs` | 1,162 | Event handling, business logic |
| `agent.rs` | 545 | Agent config, KDL I/O |
| `utils.rs` | 409 | Path utilities, helpers |
| `ui.rs` | 333 | Rendering, mode definitions |
| `model.rs` | 304 | State container |
| `main.rs` | 111 | Plugin entry point |
| `error.rs` | 58 | Error types |
| `lib.rs` | 11 | Module exports |

**Total**: ~2,933 lines
