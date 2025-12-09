# Maestro Code Review Findings

**Date:** 2025-12-09
**Reviewer:** Code Review (Rust Expert)
**Codebase:** ~3,200 lines across 12 Rust files

## Executive Summary

This is a well-structured Rust codebase for a Zellij plugin. The code demonstrates solid organization, comprehensive error handling, and good test coverage. Below are the real issues identified, ordered by importance.

---

## 1. Functionality Issues

### 1.1 CRITICAL: Lossy Args Round-Trip in Edit Flow **(Fixed)**

**Location:** `src/handlers/forms.rs:54`

```rust
model.agent_form.args = agent.args.join(" ");
```

When editing an agent, args are joined with spaces for display. But if an arg contains spaces (e.g., `["--prompt", "hello world"]`), the join produces `--prompt hello world`, and `shell_words::split` on save returns `["--prompt", "hello", "world"]`. **Data corruption occurs.**

**Example:**
- User creates agent with args: `--prompt "hello world"`
- Stored as: `["--prompt", "hello world"]`
- Displayed in edit form: `--prompt hello world`
- On save: `["--prompt", "hello", "world"]` ← **Wrong**

**Fix:** Use `shell_words::join()` to escape args when populating the edit form. Implemented.

---

### 1.2 Focus Closes Plugin Even on Error **(Fixed)**

**Location:** `src/handlers/keys.rs:36-40`

```rust
BareKey::Enter => {
    let idx = model.selected_pane;
    focus_selected(model, idx);
    close_self();  // Called unconditionally
}
```

If `focus_selected` fails (e.g., pane ID unavailable), it sets `error_message`, but `close_self()` is called anyway—hiding the error from the user. Now `close_self()` runs only when no error is present after focus.

---

### 1.3 Truncate Off-By-One

**Location:** `src/utils.rs:122-135`

```rust
pub fn truncate(s: &str, max: usize) -> String {
    // ...
    for (i, ch) in s.chars().enumerate() {
        if i >= max {
            out.push('…');  // Adds char AFTER reaching max
            break;
        }
        out.push(ch);
    }
    out
}
```

`truncate("hello", 3)` returns `"hel…"` (4 characters). The caller expects max chars but gets max+1 when truncation happens. Could cause UI misalignment in edge cases.

---

### 1.4 Permission State Can Be Invalid

**Location:** `src/model.rs:53-54`

```rust
pub permissions_granted: bool,
pub permissions_denied: bool,
```

Two separate booleans can represent invalid states (both true, or neither). Should be a single enum:

```rust
pub enum PermissionState { Pending, Granted, Denied }
```

---

## 2. Dead/Unused Code

### 2.1 `quick_launch_agent` Field Never Used

**Location:** `src/model.rs:41`

```rust
pub quick_launch_agent: Option<String>,
```

This field is defined in `PaneWizard` but never read or written outside of `Default` initialization.

---

### 2.2 `agent_form.source` Field Unused

**Location:** `src/model.rs:15` and `src/handlers/forms.rs:38,58`

The `source` field is set in `start_agent_create` and `start_agent_edit` but never read meaningfully. On Esc, the code always returns to `Mode::View` regardless of source:

```rust
// forms.rs:38
model.agent_form.source = Some(Mode::View);  // Always View, even when coming from AgentConfig
```

Either implement the return-to-source behavior or remove the field.

---

### 2.3 `serde_json` Dependency Unused

**Location:** `Cargo.toml:19`

```toml
serde_json = "1"
```

Grep shows no `serde_json` usage in the source. Config uses KDL. The dependency can be removed unless it's a transitive requirement of zellij-tile.

---

### 2.4 `_rows` Parameter Unused

**Location:** `src/ui.rs:77`

```rust
pub fn render_ui(model: &Model, _rows: usize, cols: usize) -> String {
```

The rows parameter is accepted but unused—could enable viewport-aware rendering in future.

---

## 3. UX Inconsistencies

### 3.1 Delete Confirmation Returns to Wrong Mode

**Location:** `src/handlers/keys.rs:249`

After confirming delete in `handle_key_event_delete_confirm`:
```rust
model.mode = Mode::View;
```

User came from AgentConfig to delete an agent. After confirming, they're sent to View instead of back to AgentConfig. Forces unnecessary re-navigation.

---

### 3.2 Tab Key Not Mentioned in Status Hints

**Location:** `src/ui.rs:335`

```rust
Mode::NewPaneWorkspace => "↑/↓ select • Enter continue • Esc cancel",
```

Tab is the autocomplete key but isn't mentioned in the hints. Should be:
```rust
"Tab complete • ↑/↓ select • Enter continue • Esc cancel"
```

---

### 3.3 Error Clearing Inconsistent

- `move_pane_selection` and `move_agent_selection` clear errors on navigation
- Form field navigation (Up/Down in agent form) preserves errors
- Some handlers use `reset_status()`, others use `model.error_message.clear()`

Pick one pattern and apply consistently.

---

## 4. Maintainability

### 4.1 Duplicated Duplicate-Name Check

Duplicate agent name validation exists in three places:
- `src/agent.rs:186-202` (`validate_agents`)
- `src/handlers/forms.rs:116-122` (`apply_agent_create`)
- `src/handlers/forms.rs:131-138` (`apply_agent_edit`)

Consider a single `is_name_available(agents, name, exclude_idx)` helper.

---

### 4.2 Default Agent Check Inconsistent

Multiple ways to identify default agents:
- `is_default_agent(name)` function with hardcoded match
- `default_agents()` returning a Vec
- Could derive from one source of truth

```rust
// Current: hardcoded in is_default_agent
matches!(name.trim().to_lowercase().as_str(), "cursor" | "claude" | "gemini" | "codex")

// Better: derive from default_agents()
const DEFAULT_AGENT_NAMES: &[&str] = &["cursor", "claude", "gemini", "codex"];
```

---

### 4.3 Clone in `build_command`

**Location:** `src/utils.rs:138-142`

```rust
pub fn build_command(agent: &Agent) -> Vec<String> {
    let mut parts = vec![agent.command.clone()];
    parts.extend(agent.args.clone());  // Clones entire vec
    parts
}
```

Could use `agent.args.iter().cloned()` instead of cloning the whole Vec, though this is micro-optimization.

---

## 5. Minor/Stylistic

### 5.1 Inconsistent Empty String Checks

Some places use `is_empty()`:
```rust
// ui.rs:96
if pane.agent_name.is_empty() {
```

Others use `trim().is_empty()`:
```rust
// forms.rs:88
if model.agent_form.note.trim().is_empty() {
```

Be consistent—whitespace-only agent names would pass `is_empty()` but fail trim check.

---

## What's Done Well

- **Error handling:** Comprehensive `MaestroError` enum with `thiserror`
- **Test coverage:** 100+ tests covering edge cases, shell quoting, case sensitivity
- **Module organization:** Clear separation between model, handlers, UI, utils
- **Bounds checking:** Consistent use of `clamp_selections()` after list modifications
- **Case-insensitive matching:** Properly implemented throughout for agent names
- **Shell-words parsing:** Correct handling of quoted arguments in new agents
- **WASI path handling:** Clean abstraction over `/host` mount prefix

---

## Summary Table

| Category | Issue | Severity | Location |
|----------|-------|----------|----------|
| Functionality | Args lossy round-trip in edit | **High** | `forms.rs:54` |
| Functionality | Focus closes on error | Medium | `keys.rs:36-40` |
| Functionality | Truncate off-by-one | Low | `utils.rs:122-135` |
| Functionality | Permission state invalid | Low | `model.rs:53-54` |
| Dead Code | `quick_launch_agent` unused | Low | `model.rs:41` |
| Dead Code | `source` field unused | Low | `model.rs:15` |
| Dead Code | `serde_json` dependency | Low | `Cargo.toml:19` |
| Dead Code | `_rows` parameter unused | Low | `ui.rs:77` |
| UX | Delete returns to wrong mode | Medium | `keys.rs:249` |
| UX | Tab hint missing | Low | `ui.rs:335` |
| UX | Error clearing inconsistent | Low | Various |
| Maintainability | Duplicate validation logic | Low | `agent.rs`, `forms.rs` |
| Maintainability | Default agent check inconsistent | Low | `agent.rs` |

---

## Recommendations

1. **Priority 1:** Fix the args round-trip bug—it causes data corruption
2. **Priority 2:** Fix focus-on-error behavior to show error before closing
3. **Priority 3:** Return to AgentConfig after delete confirmation
4. **Priority 4:** Clean up dead code (`quick_launch_agent`, `source`, `serde_json`)
5. **Priority 5:** Add Tab hint to workspace input status line
