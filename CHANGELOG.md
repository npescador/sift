# Changelog

All notable changes to Sift will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.2.0] — 2026-04-07

iOS developer workflow expansion. Five new filters covering the most token-expensive commands in a daily Xcode/Swift workflow, plus transparent shell hooks for zero-friction adoption.

### Added

**Shell hooks & AI agent integration**
- `sift init --shell` — injects idempotent marker-based hook functions into `~/.zshrc` / `~/.bashrc`, wrapping `git`, `xcodebuild`, `xcrun`, and `swiftlint` so all invocations are auto-filtered without typing `sift`
- `sift init --claude` — creates / updates `CLAUDE.md` with sift usage instructions for Claude Code
- `sift init --copilot` — creates / updates `.github/copilot-instructions.md` for GitHub Copilot
- `sift init --show` — displays installation status for all three integration targets
- `sift init --uninstall` — removes all sift-managed blocks from rc file and instruction files

**New filters**
- `xcrun simctl list` — iOS-only compact view (Booted first, short UDID, `3rd gen` shortening); ~92% reduction vs full output
- `xcodebuild -showBuildSettings` — extracts 16 high-signal iOS keys (bundle ID, Swift version, deployment target, signing, SDK, team…) from ~400-line output; ~95% reduction
- `git log` — one line per commit: `SHORT_HASH  subject  (date)  author`; verbose adds full hash + body preview; `--oneline` input passes through unchanged
- `swiftlint` / `swiftlint lint` — violations grouped by rule name, errors before warnings, count per rule; clean run shows `✓` summary; verbose adds top-3 file locations per rule

**Command detection**
- `xcrun` family added to `CommandFamily` with `SimctlList` / `Other` subcommands
- `swiftlint` added as a top-level `CommandFamily` variant
- `git log` added to `GitSubcommand`
- `xcodebuild -showBuildSettings` added to `XcodebuildSubcommand`

### Changed
- `CommandFamily::name()` extended with `"xcrun"` and `"swiftlint"` for tracking
- Test count: 50 → 97 (88 unit + 9 integration)

---

## [0.1.0] — 2026-04-06

First MVP release. All core command filters, config file support, and persistent tracking are implemented.

### Added

**CLI & core**
- `sift <command> [args]` proxy — runs any command and filters its output
- `sift stats` — show accumulated byte savings by command family
- Verbosity flags: `-v`, `-vv`, `-vvv`, `--raw`
- Exit code contract: subprocess exit code always propagated exactly
- Unknown commands pass through unmodified (safe passthrough)

**Filters**
- `git status` — staged / modified / untracked grouped summary with counts; compact caps at 3 files per group
- `git diff` — per-file `+N -N` stats with ANSI color; verbose adds `@@` hunk headers
- `grep` / `rg` — results grouped by file (BTreeMap), capped at 3 matches/file and 30 total in compact mode
- `cat` / `head` / `tail` / `less` — truncation at 100 lines (compact) / 200 lines (verbose); binary file detection
- `xcodebuild build` — errors grouped by file, warning count, `BUILD SUCCEEDED/FAILED` header; path shortening
- `xcodebuild test` — pass/fail/skip counts, failed test names, `XCTAssert` failure messages; verbose adds file location

**Config**
- Loads `~/.config/sift/config.toml` (`$XDG_CONFIG_HOME` aware)
- `[defaults] verbosity` — sets default verbosity level
- `[defaults] max_lines` — future truncation cap (reserved)
- `[tracking] enabled` — gates stats recording
- Verbosity priority: `--raw` > `-v` flags > config default > built-in default (compact)

**Tracking**
- Persistent stats file at `~/.local/share/sift/stats.toml` (`$XDG_DATA_HOME` aware)
- Records: command family, original bytes, filtered bytes, exit code, duration, timestamp
- `sift stats` displays: invocations, bytes saved, average reduction %, breakdown by command

**Infrastructure**
- GitHub Actions CI: `cargo test`, `cargo fmt --check`, `cargo clippy -D warnings` on `macos-latest`
- Branch protection on `main` and `develop`; squash-merge strategy
- 50 tests: 41 unit tests across all modules + 9 end-to-end integration tests
- `AGENTS.md` — integration guide for Copilot CLI, Codex CLI, and Claude Code

---

[Unreleased]: https://github.com/npescador/sift/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/npescador/sift/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/npescador/sift/releases/tag/v0.1.0
