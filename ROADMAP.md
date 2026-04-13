# Roadmap

This document tracks Sift's milestones. Each milestone represents a coherent set of capabilities released together.

For granular progress, see the [GitHub Issues](https://github.com/npescador/sift/issues) tracker.

---

## v0.1.0 — Foundation ✅ Released

Core infrastructure and basic command filtering support.

- [x] Repository setup and meta-files
- [x] Rust project scaffold (`sift-cli` crate)
- [x] CLI entry point with `clap` (subcommands, verbosity flags, `--raw`)
- [x] Executor layer (subprocess spawn, stdout/stderr/exit-code capture)
- [x] Command detection and routing (`CommandFamily` enum)
- [x] `git status` filter — grouped by file state with counts
- [x] `git diff` filter — per-file stats, hunk header summaries
- [x] `grep` / `rg` filter — grouped by file, deduplication, result cap
- [x] `cat` / `read` filter — safe truncation, configurable line ranges
- [x] Config file support (`~/.config/sift/config.toml`)
- [x] Tracking abstraction (in-memory, session-scoped)
- [x] Unit tests for all filters
- [x] README and documentation

---

## v0.2.0 — Xcode Support ✅ Released

First-class iOS/macOS developer workflow support.

- [x] `xcodebuild build` filter — compiler errors grouped by file, warning count
- [x] `xcodebuild test` filter — pass/fail counts, failed test names and assertions
- [x] `xcodebuild -showBuildSettings` filter — 16 high-signal iOS keys from ~400-line output
- [x] `xcrun simctl list` filter — iOS-only compact view, Booted first
- [x] `git log` filter — one line per commit: hash · subject · date · author
- [x] `swiftlint` filter — violations grouped by rule, errors before warnings
- [x] `sift init` — shell hooks, CLAUDE.md, copilot-instructions.md, uninstall

---

## v0.3.0 — iOS Toolchain Expansion ✅ Released

Wider coverage of the daily iOS/Swift developer workflow.

- [x] `fastlane` filter — lane name, step progression, result + total time
- [x] `xcodebuild archive` filter — result, signing identity, archive path
- [x] `swift package resolve/update/show-dependencies` filter

---

## v0.4.0 — Xcode Workflow Polish ✅ Released

Remaining high-value Xcode workflow items.

- [x] `xcodebuild -list` filter — default scheme ★, configurations, target count
- [x] `xcodebuild build` — linker errors 🔗 and signing errors 🔐 surfaced above compiler errors
- [x] `git log --graph` — decoration lines stripped, compact log format preserved
- [x] `ls` / `find` — Xcode-relevant files only; drops `.build/`, `DerivedData/`, `.o`
- [x] Tee mode — fallback to raw output when filter produces empty result; saves raw to disk

---

## v0.5.0 — iOS/AI Workflow Expansion ✅ Released

Extended command coverage for AI-assisted iOS development workflows. SQLite persistence.

**New command families (13)**
- [x] `swift build` — compiler errors grouped by file, `BUILD SUCCEEDED/FAILED`
- [x] `swift test` — pass/fail counts, failed test names and assertions (SPM)
- [x] `curl` — HTTP status, key headers, body truncated to N lines
- [x] `pod install` / `pod update` — one pod per line, warnings, result
- [x] `swiftformat` — files changed, result summary, lint errors
- [x] `tuist generate/fetch/cache` — targets, dependencies, errors
- [x] `codesign` — signing status, identifier, team
- [x] `security find-identity` — valid identities with short hash and name
- [x] `agvtool` — current/new version number, files updated
- [x] `xcode-select` — active Xcode version and path
- [x] `xcrun simctl boot/install/launch/erase/delete` — compact operation result
- [x] `xcresulttool` — test summary from `.xcresult` bundles
- [x] `docc convert/preview` — symbols processed, warnings, output path

**Persistent stats (Milestone 4)**
- [x] SQLite persistence (`rusqlite`) — `~/.local/share/sift/stats.db`
- [x] `sift stats` — multi-session historical summaries
- [x] `sift stats --last N` — last N invocations
- [x] `sift stats --reset` — clear all history
- [x] `sift stats --json` — export full history as JSON
- [x] Automatic migration from legacy `stats.toml` on first run

---

## v0.6.0 — JSON Output & Programmatic API ✅ Released

Structured filter output and streaming executor for long-running commands.

- [x] `--json` output mode for all command families
- [x] Stable, versioned JSON schema per command family (versioned envelope `{"version":1,...}`)
- [x] 38 typed structs in `src/filters/types.rs` — all implementing `serde::Serialize`
- [x] All 27 command families refactored to parse/render pattern
- [x] Streaming executor (`src/streaming.rs`) — live progress to stderr for builds and tests
- [x] Shared filter utilities (`src/filters/util.rs`)
- [x] `insta` snapshot regression tests + error recall framework
- [x] Shell hook hardening: CI detection, `--commands` opt-in flag
- [ ] Programmatic library API (`sift-lib` crate) — deferred to v0.7.0

---

## v0.7.0 — Shell Completions & sift-lib 🚧 In Progress

Quality-of-life improvements for daily use, plus the programmatic crate API.

- [x] Shell completion scripts (zsh, bash, fish) — `sift completions <shell>`
- [x] `sift init --completions <shell>` — auto-install to standard location
- [x] Per-command override configuration (`[commands.git]` etc.)
- [x] `sift-lib` crate — programmatic library API for embedding Sift in other tools
- [x] `sift benchmark` command for measuring real-world token savings
- [ ] `sift update` — self-update from GitHub releases

---

## v1.0.0 — Stable Release 🔮 Planned

First stable release with full documentation and compatibility guarantees.

- [ ] Stable public API contract
- [ ] Full documentation coverage
- [ ] Windows support
- [ ] Performance benchmarks and regression tests

---

## Non-Goals (V1)

- GUI or web UI
- Remote or cloud execution
- Cloud telemetry (all tracking is always local)
- AI-based summarization (Sift uses rule-based filters by design for predictability)


---

## Milestone 1 — Foundation (MVP) 🚧 In Progress

Core infrastructure and basic command filtering support. The goal is a working `sift` binary that adds real value for the primary command families used in iOS/macOS development.

- [x] Repository setup and meta-files
- [ ] Rust project scaffold (`sift-cli` crate)
- [ ] CLI entry point with `clap` (subcommands, verbosity flags, `--raw`)
- [ ] Executor layer (subprocess spawn, stdout/stderr/exit-code capture)
- [ ] Command detection and routing (`CommandFamily` enum)
- [ ] `git status` filter — grouped by file state with counts
- [ ] `git diff` filter — per-file stats, hunk header summaries
- [ ] `grep` / `rg` filter — grouped by file, deduplication, result cap
- [ ] `cat` / `read` filter — safe truncation, configurable line ranges
- [ ] Config file support (`~/.config/sift/config.toml`)
- [ ] Tracking abstraction (in-memory, session-scoped)
- [ ] Unit tests for all filters
- [ ] README and documentation pass

---

## Milestone 2 — Xcode Support 🍎

First-class iOS/macOS developer workflow support. This is Sift's primary differentiator.

- [ ] `xcodebuild build` filter — group unique compiler errors by file, summarize warnings
- [ ] `xcodebuild test` filter — pass/fail counts, failed test names and errors
- [ ] Swift compiler error message normalization
- [ ] Warning deduplication across build targets
- [ ] Simulator output noise reduction
- [ ] Integration test fixtures from real `xcodebuild` runs

---

## Milestone 3 — Shell Integration

Make Sift seamlessly transparent in daily terminal workflows.

- [ ] Shell function wrappers (zsh, bash, fish)
- [ ] `sift install-hooks` command for automated shell integration
- [ ] Per-command alias strategy documentation
- [ ] Safe uninstall / rollback path
- [ ] Shell completion scripts (zsh, bash, fish)

---

## Milestone 4 — Metrics & Persistence

Quantify Sift's value with real, persistent data.

- [ ] SQLite persistence (`rusqlite`) for tracking records
- [ ] `sift stats` subcommand — per-session and historical summaries
- [ ] Per-command-family savings breakdown
- [ ] `sift stats --reset` to clear history
- [ ] Export to JSON for external analysis

---

## Milestone 5 — AI Agent Integrations

Native integration with leading AI coding tools, with documented workflows and benchmarks.

- [ ] `AGENTS.md` for Codex CLI compatibility
- [ ] Copilot CLI workflow guide
- [ ] Claude Code integration guide
- [ ] Automated token savings benchmark suite
- [ ] `sift benchmark` command for measuring real-world savings

---

## Milestone 6 — JSON Output & Programmatic API

Enable machine-readable output for advanced agent integrations.

- [ ] `--json` output mode for all command families
- [ ] Stable, versioned JSON schema per command family
- [ ] Programmatic library API (`sift-lib` crate) for embedding in other tools

---

## Non-Goals (V1)

The following are explicitly out of scope for the first major version:

- GUI or web UI
- Remote or cloud execution
- Cloud telemetry (all tracking is always local)
- Windows support (planned for a future major version)
- AI-based summarization (Sift uses rule-based filters by design for predictability)
