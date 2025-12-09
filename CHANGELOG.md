# Changelog
All notable changes to this project are documented here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2025-12-09
- _No changes yet._

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

[Unreleased]: https://github.com/richhaase/maestro/compare/v0.1.4...HEAD
[0.1.4]: https://github.com/richhaase/maestro/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/richhaase/maestro/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/richhaase/maestro/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/richhaase/maestro/releases/tag/v0.1.1
