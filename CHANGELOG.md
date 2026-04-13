# Changelog

All notable changes to Sift will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.6.0] — 2026-04-13

Structured filter output and streaming executor. All 27 command families now support `--json` output via typed intermediate representations. Long-running build and test commands stream live progress to stderr.

### Added

**Structured filter types (`src/filters/types.rs`)**
- 38 typed structs across all command families, all implementing `serde::Serialize`
- Shared primitives: `Diagnostic`, `Severity`, `TestFailure`
- Full coverage: `XcodebuildBuildResult`, `XcodebuildTestResult`, `XcodebuildArchiveResult`, `XcodebuildListResult`, `XcodebuildSettingsResult`, `SwiftBuildResult`, `SwiftTestResult`, `SwiftPackageResult`, `GitStatusResult`, `GitDiffResult`, `GitLogResult`, `SwiftlintResult`, `SwiftFormatResult`, `SimctlListResult`, `XcresultResult`, `PodResult`, `FastlaneResult`, `CodesignResult`, `SecurityIdentityResult`, `CurlResult`, `GrepResult`, `LsResult`, `DoccResult`, `TuistResult`, `AgvtoolResult`, `XcodeSelectResult`, `ReadResult`

**JSON output (`--json`)**
- `sift --json <command>` emits a versioned JSON envelope: `{"version": 1, "command": "...", "data": {...}}`
- All 27 command families emit fully typed JSON; unknown commands degrade gracefully to `{"raw": "..."}`
- Stable, machine-readable output for AI agent integrations and programmatic consumers

**Streaming executor (`src/streaming.rs`)**
- Live progress to stderr for long-running builds and tests — stdout remains clean and unaffected
- `xcodebuild build` / `archive` / `swift build`: compilation progress per module, first 5 errors surfaced inline with `…more errors (will summarize at end)` after the threshold
- `xcodebuild test` / `swift test`: each test result streamed as it runs — `✓` pass / `✗` fail

**Shared filter utilities (`src/filters/util.rs`)**
- `short_path(path, keep)` — trims long file paths to last N components
- `plural(n)` — `""` or `"s"` for English plurals
- `split_at_marker(line, marker)` — splits compiler diagnostic lines at severity markers (e.g. `": error:"`)

**Regression safety**
- `insta` snapshot tests (`tests/snapshot_tests.rs`) — catch unintended rendering changes across all converted filters
- Error recall framework (`tests/recall.rs`) — `.errors.json` manifests per filter guarantee that critical error signals are never silently dropped

**Shell hook hardening (`sift init --shell`)**
- CI environment detection — hooks auto-disable in CI (`CI`, `GITHUB_ACTIONS`, `JENKINS_URL` env vars)
- `--commands` flag — opt-in to wrap specific commands only (e.g. `sift init --shell --commands git,xcodebuild`)

### Changed
- All 27 command families refactored to parse/render pattern — clean separation between parsing raw output and rendering text or JSON
- Test count: 292 → 715 (322 unit + 366 filter + 9 integration + 8 recall + 10 streaming)

---

## [0.5.0] — 2026-04-07

iOS/AI workflow expansion. Thirteen new command families covering the full daily iOS development lifecycle, plus SQLite-backed persistent statistics.

### Added

**New filters**
- `swift build` — compiler errors grouped by file, `BUILD SUCCEEDED/FAILED` header; ~90% reduction
- `swift test` (SPM) — pass/fail counts, failed test names and XCTAssert messages; ~90% reduction
- `curl` — HTTP status line, key response headers (`content-type`, `content-length`, `location`), body truncated to 20 lines; ~85% reduction
- `pod install` / `pod update` — one pod per line with version, warnings surfaced, result summary; ~80% reduction
- `swiftformat` — changed files listed, result summary line, lint errors in lint mode; ~75% reduction
- `tuist generate` / `fetch` / `cache` — targets generated, dependencies resolved, errors; ~70% reduction
- `codesign` — signing status, identifier, team, format; errors shown as-is; ~80% reduction
- `security find-identity` — valid identities with short hash (first 8 chars) and name; ~80% reduction
- `agvtool` — `what-version` / `new-version` / `bump-versions`: current/new number, files updated; ~85% reduction
- `xcode-select` — active Xcode version and path for `--version` / `--print-path`; ~70% reduction
- `xcrun simctl boot/install/launch/erase/delete` — compact operation result; ~75% reduction
- `xcresulttool` — test summary from `.xcresult` bundles; designed for CI agents; ~90% reduction
- `docc convert` / `preview` — symbols processed count, warnings, output path; ~75% reduction

**Persistent stats (Milestone 4)**
- SQLite persistence via `rusqlite` — records stored in `~/.local/share/sift/stats.db`
- `sift stats` now shows multi-session historical totals (previously session-only)
- `sift stats --last N` — show stats for the last N invocations only
- `sift stats --reset` — delete all historical records
- `sift stats --json` — export full record history as JSON (via `serde_json`)
- Automatic migration: if `stats.toml` exists from a previous version, records are imported into SQLite on first run and the file is renamed to `stats.toml.bak`

**Command detection**
- `swift build` / `swift test` — new `CommandFamily::SwiftBuild` with `SwiftBuildSubcommand`
- `curl` — new `CommandFamily::Curl`
- `pod` — new `CommandFamily::Pod` with `PodSubcommand` (Install, Update)
- `swiftformat` — new `CommandFamily::SwiftFormat`
- `tuist` — new `CommandFamily::Tuist` with `TuistSubcommand` (Generate, Fetch, Cache, Edit)
- `codesign` — new `CommandFamily::Codesign`
- `security` — new `CommandFamily::Security`
- `agvtool` — new `CommandFamily::Agvtool`
- `xcode-select` — new `CommandFamily::XcodeSelect`
- `xcresulttool` — new `CommandFamily::XcResultTool`
- `docc` — new `CommandFamily::DocC`
- `xcrun simctl` — extended with `SimctlBoot`, `SimctlInstall`, `SimctlLaunch`, `SimctlErase`, `SimctlDelete` subcommands

**Documentation**
- `README.md` — full rewrite: 31 commands across 7 categories, `sift init` section, updated stats flags, tee mode docs, correct license attribution
- `ROADMAP.md` — rewritten to show actual completion status per version; planned v0.6.0 and v0.7.0 added
- `ARCHITECTURE.md` — updated module tree (44 source files), correct `CommandFamily` enum, correct `FilterOutput` struct, updated dependencies table
- `AGENTS.md` — updated milestones table, expanded command reduction table, new `sift stats` flags
- `CONTRIBUTING.md` — corrected filter routing instructions (dispatch is in `main.rs::apply_filter`)
- `LICENSE` — copyright updated to `Nacho Pescador Ruiz`

### Changed
- Test count: 289 → 292 (unit + integration)
- `Cargo.toml`: added `rusqlite = { version = "0.31", features = ["bundled"] }` and `serde_json = "1"`

---

## [0.4.0] — 2026-04-07

Xcode workflow polish. Five new capabilities covering the remaining high-value items in the v0.4.0 roadmap.

### Added

**New filters**
- `xcodebuild -list` — project name, schemes (★ default), configurations, target count; verbose lists all targets; ~60% reduction
- `xcodebuild build` improvements — linker errors (🔗 `ld:`, `Undefined symbols`, `clang: error: linker`) and signing/provisioning errors (🔐) detected and surfaced above compiler errors; ordering: signing → linker → compiler
- `git log --graph` — detects `--graph` flag, strips decoration lines (`*`, `|`, `/`, `\`), delegates to existing compact log format; works for both `--oneline --graph` and multi-line `--graph`
- `ls` / `find` for Xcode — filters output to Xcode-relevant files (`.swift`, `.xcodeproj`, `.plist`, `Package.swift`, etc.); drops `.build/`, `DerivedData/`, `.o`, `.a`, `.DS_Store`; directories always preserved

**Tee mode**
- When a filter produces empty output from non-empty input (possible false negative), Sift falls back to raw output and saves the raw to `~/.local/share/sift/raw/<timestamp>-<cmd>.txt`
- Warning printed to stderr: `[sift] filter produced empty output — raw saved to <path>`
- Configurable via `[tee] enabled = true/false` in `~/.config/sift/config.toml`

**Command detection**
- `XcodebuildSubcommand::List` added for `-list` flag
- `GitSubcommand::LogGraph` added — activated when `--graph` appears anywhere in args
- `CommandFamily::Ls` — detects `ls`, `eza`, `exa`
- `CommandFamily::Find` — detects `find`

---

## [0.3.0] — 2026-04-07

iOS toolchain expansion. Four new filters covering the remaining high-token commands in a daily iOS/Swift developer workflow.

### Added

**New filters**
- `fastlane` — compact lane execution summary: lane name, warnings/errors, result + total time; verbose adds step-by-step progression with `(N/M)` tracking; strips timestamps and ANSI codes; ~85% reduction
- `xcodebuild archive` — `ARCHIVE SUCCEEDED/FAILED` header + scheme/config + archive path (📦) + signing team (🔑) + identity (🔐) + errors grouped by file; verbose adds warning count; ~95% reduction
- `swift package resolve` / `update` / `show-dependencies` — one line per package (name + version); verbose adds source URL; detects `show-dependencies` dependency tree; ~80% reduction
- `git log` dynamic year: `compact_date()` now uses `SystemTime` for current year instead of hardcoded 2026

**Command detection**
- `fastlane` added to `CommandFamily`
- `xcodebuild archive` added to `XcodebuildSubcommand`
- `SwiftPackage(SwiftPackageSubcommand)` added to `CommandFamily` — detects `swift package resolve/update/show-dependencies`

**Developer experience**
- `AGENTS.md` updated with commit message rules (one line, `type: message`, no trailers) and post-merge workflow (checkout develop, pull, delete branch, prune remotes)
- `AGENTS.md` updated with PR creation reference (labels, milestones, base branch)

### Changed
- Test count: 109 → 137 (128 unit + 9 integration)

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

[Unreleased]: https://github.com/npescador/sift/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/npescador/sift/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/npescador/sift/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/npescador/sift/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/npescador/sift/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/npescador/sift/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/npescador/sift/releases/tag/v0.1.0
