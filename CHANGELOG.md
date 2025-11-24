# Changelog
All notable changes are tracked here following [Keep a Changelog](https://keepachangelog.com/) and Semantic Versioning once releases are tagged.

## [Unreleased]

### Features
- Added typedown autocomplete while entering workspace paths. (`ebe4293`)
- Added fuzzy "filter or create" interface to the agent selection step, plus a matching tab filter/creation flow. (`0e729e3`, `a363547`)
- Updated the Agent Config pane so `Enter` launches, `e` edits, and the section is labeled "Agent Config". (`d9efa3d`)
- Seeded default agents (cursor, claude, gemini, codex) and enhanced pane matching against their commands. (`2c4b702`)
- Expanded inline unit tests covering models, handlers, and error handling. (`61413cc`)
- Replaced ad-hoc string errors with structured error types for better user-facing messages. (`be89112`)

### Fixes
- Fixed workspace path resolution to use relative paths so Zellij opens panes in the expected directory. (`6300cbc`)
- Restored filtering so panes only display for the current session. (`d95b489`)
- Scoped agent panes to the current session to avoid cross-session leakage. (`3fc4dff`)
- Cleaned up dead code, unused imports, and clippy warnings. (`764d530`, `c3f8e3b`)
- Extracted permission rendering and event handlers to dedicated modules, reducing UI glitches during permission prompts. (`0bf6bb7`, `fc0007f`)
- Removed obsolete planning docs from `docs/` to avoid stale references. (`b244572`, `ac2951b`)

### Breaking Changes
- Changed the agent config path to be relative to the user home directory, so existing configs must be moved to `~/.config/maestro/agents.kdl`. (`1292d8a`)
- Refactored the model and agent persistence layers (Phases 1 & 2) to new module layouts; downstream forks should update module imports. (`f614b2f`, `87ae9a9`, `6cabf19`)
