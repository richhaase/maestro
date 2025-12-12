# Changelog

All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2025-12-12

### Changed

- Refreshed README with Features section, Prerequisites with versions, Installation steps reorganization, and key commands table. ([8c8daf9](https://github.com/richhaase/maestro/commit/8c8daf9))
- Clarified agent persistence behavior: full list (defaults + custom) is saved, not just custom agents. ([8c8daf9](https://github.com/richhaase/maestro/commit/8c8daf9))
- Updated Zellij version requirement to 0.43+ to match zellij-tile dependency. ([8c8daf9](https://github.com/richhaase/maestro/commit/8c8daf9))
- Code quality improvements: consistent agent name validation, simplified derive_tab_name_from_workspace, added Default derives. ([71b2560](https://github.com/richhaase/maestro/commit/71b2560))
- Improved case-insensitive agent lookup, added Tab key hint to workspace path, added Model::clear_error() method. ([3b58585](https://github.com/richhaase/maestro/commit/3b58585))
- Formatting with cargo fmt. ([882a95c](https://github.com/richhaase/maestro/commit/882a95c))

### Added

- Added comprehensive SETUP.md with installation steps, prerequisites, verification, troubleshooting, and environment configuration. ([8c8daf9](https://github.com/richhaase/maestro/commit/8c8daf9))

### Fixed

- Fixed truncate() to correctly account for ellipsis in max length calculation. ([3b58585](https://github.com/richhaase/maestro/commit/3b58585))
- Fixed license field in Cargo.toml. ([e56e407](https://github.com/richhaase/maestro/commit/e56e407))

## [0.2.0] - 2025-12-09

### Fixed

- Code cleanup and consistency improvements across handlers and utilities. ([cab074f](https://github.com/richhaase/maestro/commit/cab074f))
- Fixed agent arg editing and preserve focus errors. ([d11074e](https://github.com/richhaase/maestro/commit/d11074e))

### Changed

- Removed dead code and unused dependencies. ([ab10f2b](https://github.com/richhaase/maestro/commit/ab10f2b))
- Enforced case-insensitive agent names and improved arg error messages. ([e7621d3](https://github.com/richhaase/maestro/commit/e7621d3))
- Parse agent args with shell quoting for proper argument handling. ([ac5a2df](https://github.com/richhaase/maestro/commit/ac5a2df))
- Keep tab names in sync during session updates. ([ea6e3c6](https://github.com/richhaase/maestro/commit/ea6e3c6))
- Removed optimistic pane placeholder and exit after spawn for cleaner UX. ([e6053b3](https://github.com/richhaase/maestro/commit/e6053b3))

### Added

- Handle tab rename updates for agent panes. ([ca6d699](https://github.com/richhaase/maestro/commit/ca6d699))
- Keep selected agent after persisting changes. ([c58d552](https://github.com/richhaase/maestro/commit/c58d552))
- Session tab hydration test for improved reliability. ([86b26ea](https://github.com/richhaase/maestro/commit/86b26ea))

## [0.1.4] - 2025-12-08

### Fixes

- Match panes to agent definitions using the full command string (including args) so session rebuilds continue to track custom agents correctly. ([325f3da](https://github.com/richhaase/maestro/commit/325f3da))

## [0.1.3] - 2025-12-08

### Breaking Changes

- Removed the broken per-agent `env` field; commands now assume the first token is the executable, preventing Maestro from trying to run `KEY=value` binaries. ([de11097](https://github.com/richhaase/maestro/commit/de11097))

## [0.1.2] - 2025-12-08

### Features

- Added a dedicated `args` field to agent definitions so multi-argument commands remain ordered and editable from the UI. ([9d2cda9](https://github.com/richhaase/maestro/commit/9d2cda9))

## [0.1.1] - 2025-12-06

### Features

- Initial release with agent persistence under `~/.config/maestro/agents.kdl`, default agent templates, and end-to-end launch/focus/kill piping through Zellij panes. ([79db865](https://github.com/richhaase/maestro/commit/79db865), [026175b](https://github.com/richhaase/maestro/commit/026175b), [61413cc](https://github.com/richhaase/maestro/commit/61413cc))
- Added fuzzy workspace autocomplete and agent selection flows to the new-pane wizard for quick keyboard navigation. ([ebe4293](https://github.com/richhaase/maestro/commit/ebe4293), [0e729e3](https://github.com/richhaase/maestro/commit/0e729e3))
- Implemented pane focus/kill commands, auto-close-on-focus, and session resync logic to keep the UI accurate after reloads. ([1557358](https://github.com/richhaase/maestro/commit/1557358), [dd87d4a](https://github.com/richhaase/maestro/commit/dd87d4a), [3b6323f](https://github.com/richhaase/maestro/commit/3b6323f))

[Unreleased]: https://github.com/richhaase/maestro/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/richhaase/maestro/compare/v0.1.4...v0.2.0
[0.1.4]: https://github.com/richhaase/maestro/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/richhaase/maestro/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/richhaase/maestro/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/richhaase/maestro/releases/tag/v0.1.1
