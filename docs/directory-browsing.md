# Directory Browsing for Workspace Selection

## Overview
Add a file browser interface to allow users to visually browse and select workspace directories instead of manually typing paths.

## Current State

### Existing Flow
1. User presses `n` (new pane)
2. Enters `NewPaneWorkspace` mode
3. Types workspace path manually in text input
4. Proceeds to tab selection, then agent selection

### Problems
- Users must know exact paths
- Typo-prone
- No visual feedback about available directories
- Difficult to discover workspace locations

## Goals
- Enable visual directory browsing starting from home directory (`/host`)
- Allow navigation through directory tree
- Select directory as workspace
- Maintain fallback for manual path entry
- Keep existing keyboard-driven UI paradigm

## Design

### UI/UX Flow

**New Mode: `NewPaneWorkspaceBrowse`**
- Replaces or supplements `NewPaneWorkspace` mode
- Shows current directory path at top
- Lists directories and files in current location
- Navigation:
  - `j`/`k`: Move selection up/down
  - `Enter`: Enter selected directory (if directory) or select as workspace (if directory)
  - `h`/`Backspace`: Go up one directory level
  - `~`: Jump to home directory
  - `Esc`: Cancel browsing, return to manual entry or previous step
  - `f`: Filter/search directories (reuse existing filter mechanism)
  - `Tab`: Switch between browse view and manual entry field

**Display Format:**
```
Current: /host/projects

  📁 projects/
  📁 documents/
  📁 downloads/
  📁 .config/
  📄 README.md

[Enter] select  [h] up  [Esc] cancel  [Tab] manual entry
```

**Visual Indicators:**
- `📁` for directories
- `📄` for files (shown but not selectable as workspace)
- `>` indicator for selected item
- Show `..` at top for parent directory

### Integration Points

**Mode Transitions:**
- `NewPaneWorkspace` → `NewPaneWorkspaceBrowse` (new mode)
- `NewPaneWorkspaceBrowse` → `NewPaneTabSelect` (after selection)
- `NewPaneWorkspaceBrowse` → `NewPaneWorkspace` (if user switches to manual entry)

**State Management:**
- Add `browse_current_path: String` to Model
- Add `browse_selected_idx: usize` to Model
- Store selected directory path when user confirms

## Technical Implementation

### Prerequisites
**Zellij API Investigation:**
- Check if `ReadFiles` permission allows directory listing
- Verify available file system APIs in WASM sandbox
- Determine if we can read directory contents or need alternative approach

**Alternative Approaches if Directory Reading Not Available:**
1. **Tab Completion:** Enhance manual entry with tab completion
2. **Recent Paths:** Remember and show recently used workspace paths
3. **Config-based:** Allow users to configure favorite workspace paths
4. **Hybrid:** Combine manual entry with smart suggestions

### Implementation Steps

**Phase 1: API Research & Feasibility**
1. Research Zellij WASM file system capabilities
2. Test directory reading in sandbox
3. Document limitations and alternatives
4. Decide on approach (browsing vs completion vs hybrid)

**Phase 2: Model Updates**
1. Add `browse_current_path: String` field to Model
2. Add `browse_selected_idx: usize` field to Model  
3. Add `browse_entries: Vec<DirEntry>` field to Model (if browsing feasible)
4. Add getters/mutators for new fields

**Phase 3: Directory Reading (if feasible)**
1. Create `utils::read_directory(path: &Path) -> Result<Vec<DirEntry>>`
2. Handle errors (permission denied, not found, etc.)
3. Filter entries (show/hide hidden files based on preference)
4. Sort entries (directories first, then alphabetically)

**Phase 4: UI Rendering**
1. Create `ui::render_directory_browser(model: &Model) -> String`
2. Display current path
3. Render directory listing with selection indicator
4. Show navigation hints

**Phase 5: Navigation Logic**
1. Add `handlers::handle_browse_navigation(model: &mut Model, key: KeyWithModifier)`
2. Handle `j`/`k` for selection movement
3. Handle `Enter` for directory entry/selection
4. Handle `h`/`Backspace` for going up
5. Handle `~` for home directory
6. Handle `Tab` for switching to manual entry
7. Update `clamp_selections` to work with browse mode

**Phase 6: Integration**
1. Update `start_new_pane_workspace` to initialize browse mode
2. Add key handler for `NewPaneWorkspaceBrowse` mode
3. Update wizard flow to proceed to tab selection after directory selection
4. Maintain backward compatibility with manual entry

**Phase 7: Edge Cases & Polish**
1. Handle permission errors gracefully
2. Handle empty directories
3. Handle very long directory names (truncate in display)
4. Handle symlinks (follow or show indicator?)
5. Performance: cache directory contents?
6. Add loading indicator for slow directory reads

### Data Structures

```rust
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub name: String,
    pub is_directory: bool,
    pub is_hidden: bool,
}
```

## Edge Cases

1. **Permission Denied:**
   - Show error message
   - Allow navigation away
   - Don't crash

2. **Non-existent Path:**
   - Shouldn't happen if browsing, but handle gracefully
   - Fall back to manual entry

3. **Empty Directory:**
   - Show "Empty directory" message
   - Still allow selection

4. **Very Long Names:**
   - Truncate in display with ellipsis
   - Show full name in status/help text

5. **Symlinks:**
   - Option: Follow symlinks
   - Option: Show indicator and allow selection
   - Recommendation: Follow symlinks for simplicity

6. **Large Directories:**
   - Performance: May be slow to read
   - Consider: Virtual scrolling or pagination
   - Initial: Load all, optimize later if needed

7. **Concurrent Changes:**
   - Directory deleted while browsing
   - File added/removed
   - Handle gracefully, refresh on navigation

## Testing Strategy

1. **Unit Tests:**
   - `read_directory` function
   - Directory entry filtering/sorting
   - Path navigation logic

2. **Integration Tests:**
   - Browse navigation flow
   - Mode transitions
   - Error handling

3. **Manual Testing:**
   - Test with various directory structures
   - Test permission scenarios
   - Test with symlinks
   - Test performance with large directories

## Future Enhancements

1. **Bookmarks/Favorites:**
   - Allow users to bookmark frequently used directories
   - Quick access from browse view

2. **Recent Paths:**
   - Remember last N workspace paths
   - Show in browse view or separate section

3. **Search:**
   - Full-text search across directory tree
   - Filter by name pattern

4. **Multiple Selection:**
   - Select multiple workspaces for batch agent launch
   - (Future feature, not in scope)

## Open Questions

1. **Does Zellij WASM API support directory reading?**
   - Need to verify before implementation
   - May require alternative approach

2. **Should we show files or only directories?**
   - Recommendation: Show both, but only allow directory selection
   - Alternative: Filter to directories only

3. **Hidden files visibility:**
   - Default: Hide (`.config`, `.git`, etc.)
   - Option: Show with toggle
   - Recommendation: Hide by default, add toggle later

4. **Performance concerns:**
   - How slow is directory reading in WASM?
   - Need to test with large directories
   - May need optimization strategies

5. **Manual entry fallback:**
   - Always available via Tab?
   - Or separate mode entirely?
   - Recommendation: Tab to switch, keep both available
