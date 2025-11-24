# Tab Name Override with Default `maestro:<dir>`

## Overview
Allow users to override the auto-generated tab name when launching agents, with a simpler default format of `maestro:<directory_name>`.

## Current State

### Existing Behavior
- Tab names are auto-generated using `workspace_tab_name()` function
- Function uses workspace path to generate name
- No user control over tab name
- Name generation logic may be complex/unpredictable

### Current Function
```rust
pub fn workspace_tab_name(path: &str) -> String
```
- Generates tab name from workspace path
- May include path components, hashing, or other logic
- User cannot customize

## Goals
- Change default tab name to simple `maestro:<directory_name>` format
- Allow users to override tab name during launch flow
- Make override optional (use default if empty)
- Maintain backward compatibility
- Keep UI simple and intuitive

## Design

### Default Behavior Change

**New Default Format:**
- Pattern: `maestro:<directory_basename>`
- Example: `/home/user/projects/myapp` → `maestro:myapp`
- Example: `/home/user/docs` → `maestro:docs`
- Example: `/home/user` → `maestro:user`

**Implementation:**
- Update or replace `workspace_tab_name()` function
- Extract basename from workspace path
- Prepend `maestro:` prefix
- Handle edge cases (empty path, root directory, etc.)

### Override Mechanism

**UI Flow Options:**

**Option A: Separate Step (Recommended)**
- Add new mode: `NewPaneTabName`
- Insert between workspace selection and agent selection
- User can edit tab name or leave empty for default
- Flow: Workspace → Tab Name → Agent → Launch

**Option B: Combined with Workspace**
- Add tab name input field to workspace selection screen
- User can set both workspace and tab name together
- Simpler flow but potentially cluttered UI

**Option C: Optional Field**
- Add tab name input as optional field in existing wizard
- Show/hide based on user preference or key press
- Most flexible but potentially confusing

**Recommendation: Option A**
- Clear separation of concerns
- Explicit step makes override obvious
- Easy to skip (just press Enter with empty field)
- Consistent with existing wizard pattern

### UI/UX Design

**Tab Name Input Screen:**
```
Tab name (leave empty for default: maestro:<dir>)

maestro:myapp
                    ↑
              [editable field]

Default: maestro:myapp
[Enter] continue  [Esc] back
```

**Display:**
- Show default value as hint/placeholder
- Show actual default that will be used
- Allow editing with normal text input
- Empty field = use default
- Non-empty field = use custom name

**Keyboard Navigation:**
- Normal text editing (same as other form fields)
- `Enter`: Proceed to agent selection
- `Esc`: Go back to workspace selection
- `Tab`: Move between fields (if multiple on screen)

## Technical Implementation

### Implementation Steps

**Phase 1: Update Default Tab Name Function**
1. Modify `workspace_tab_name()` or create new `default_tab_name()` function
2. Extract basename from workspace path
3. Format as `maestro:<basename>`
4. Handle edge cases:
   - Empty path → `maestro:workspace`
   - Root directory → `maestro:root` or `maestro:/`
   - Path ending in `/` → strip trailing slash
   - Multiple slashes → normalize

**Phase 2: Model Updates**
1. Add `custom_tab_name: Option<String>` to Model (or reuse existing field if available)
2. Add getter/mutator methods
3. Initialize to `None` (use default) or empty string

**Phase 3: Add New Mode (if Option A)**
1. Add `NewPaneTabName` to `Mode` enum in `ui.rs`
2. Add rendering function `render_tab_name_input(model: &Model) -> String`
3. Show default value hint
4. Show current input value

**Phase 4: Update Wizard Flow**
1. Modify `start_new_pane_workspace` or workspace selection completion
2. Transition to `NewPaneTabName` mode instead of directly to agent selection
3. Add handler for `NewPaneTabName` mode key events
4. On Enter (with or without custom name), proceed to agent selection
5. Store custom tab name in model

**Phase 5: Update Launch Logic**
1. Modify `spawn_agent_pane()` function
2. Check for custom tab name in model
3. Use custom name if present, otherwise use default
4. Pass tab name to `new_tab()` call
5. Clear custom tab name after launch

**Phase 6: Integration**
1. Update `start_agent_create` or similar if needed
2. Ensure custom tab name is cleared when canceling wizard
3. Update status messages to reflect custom tab names

### Code Changes

**New/Modified Functions:**

```rust
// utils.rs
pub fn default_tab_name(workspace_path: &str) -> String {
    let basename = workspace_basename(workspace_path);
    if basename.is_empty() {
        "maestro:workspace".to_string()
    } else {
        format!("maestro:{}", basename)
    }
}

// handlers.rs - Update spawn_agent_pane
pub fn spawn_agent_pane(
    model: &mut Model,
    workspace_path: String,
    agent_name: String,
    tab_choice: TabChoice,
) {
    // ... existing code ...
    
    let tab_name = model.custom_tab_name()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| default_tab_name(&workspace_path));
    
    // Use tab_name instead of workspace_tab_name(&workspace_path)
    // ...
}
```

**Model Updates:**

```rust
// model.rs
pub struct Model {
    // ... existing fields ...
    custom_tab_name: Option<String>,
}

impl Model {
    pub fn custom_tab_name(&self) -> Option<&String> { ... }
    pub fn custom_tab_name_mut(&mut self) -> &mut Option<String> { ... }
}
```

**Mode Updates:**

```rust
// ui.rs
pub enum Mode {
    // ... existing modes ...
    NewPaneTabName,
}
```

## Edge Cases

1. **Empty Custom Name:**
   - Treat as "use default"
   - Clear custom_tab_name or set to None
   - Use default_tab_name()

2. **Whitespace-only Name:**
   - Trim whitespace
   - If empty after trim, use default
   - Otherwise use trimmed value

3. **Invalid Characters:**
   - Tab names may have restrictions
   - Validate or sanitize?
   - Show error if invalid?
   - Recommendation: Allow, let Zellij handle validation

4. **Very Long Names:**
   - Zellij may truncate
   - No need to limit in our code
   - Show warning if extremely long?

5. **Special Characters:**
   - Directory names may contain special chars
   - Default name should handle these
   - Custom name: user's responsibility

6. **Duplicate Tab Names:**
   - Zellij handles this (renames or errors)
   - No special handling needed
   - User can see conflict and adjust

7. **Workspace Path Changes:**
   - If user goes back and changes workspace
   - Update default tab name hint
   - Clear or keep custom name?
   - Recommendation: Keep custom name unless user clears it

## Backward Compatibility

**Considerations:**
- Existing code may call `workspace_tab_name()` directly
- Need to audit all call sites
- Update to use new default or keep old function as deprecated
- Recommendation: Update all call sites, remove old function

**Migration:**
- No data migration needed (no persisted tab names)
- Code changes only
- Test all launch paths

## Testing Strategy

1. **Unit Tests:**
   - `default_tab_name()` function
   - Edge cases (empty, root, trailing slash, etc.)
   - Basename extraction

2. **Integration Tests:**
   - Wizard flow with custom name
   - Wizard flow with default name
   - Tab name in spawned panes

3. **Manual Testing:**
   - Test with various workspace paths
   - Test with special characters
   - Test empty custom name
   - Test very long custom names
   - Test duplicate tab names

## Future Enhancements

1. **Tab Name Templates:**
   - Allow templates like `maestro:{dir}-{agent}`
   - User-configurable templates
   - (Future feature)

2. **Tab Name History:**
   - Remember recently used custom tab names
   - Suggest from history
   - (Future feature)

3. **Validation & Preview:**
   - Show preview of final tab name
   - Validate before proceeding
   - (Future feature)

4. **Per-Agent Defaults:**
   - Allow agents to have default tab name patterns
   - Override global default
   - (Future feature)

## Open Questions

1. **Should we validate tab names?**
   - Recommendation: No, let Zellij handle it
   - But could show warning for obviously invalid names

2. **What if user wants no prefix?**
   - Allow empty prefix in custom name?
   - Or require `maestro:` prefix?
   - Recommendation: Allow any name, no restrictions

3. **Should default be configurable?**
   - Global config for default pattern?
   - Or hardcode `maestro:<dir>`?
   - Recommendation: Hardcode for now, make configurable later if needed

4. **Tab name in existing panes:**
   - Should we allow renaming existing tabs?
   - Out of scope for this feature
   - But consider for future

5. **Integration with directory browsing:**
   - When browsing, show preview of default tab name?
   - Update as user navigates?
   - Nice-to-have enhancement
