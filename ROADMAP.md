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

## v0.7.0 — Shell Completions & sift-lib ✅ Released

Quality-of-life improvements for daily use, plus the programmatic crate API.

- [x] Shell completion scripts (zsh, bash, fish) — `sift completions <shell>`
- [x] `sift init --completions <shell>` — auto-install to standard location
- [x] Per-command override configuration (`[commands.git]` etc.)
- [x] `sift-lib` crate — programmatic library API for embedding Sift in other tools
- [x] `sift benchmark` command for measuring real-world token savings
- [x] `sift update` — self-update from GitHub releases

---

## v0.8.0 — iOS Intelligence 🔮 Planned

Deep iOS project introspection and crash analysis. Focused on the commands AI agents use obsessively but developers never read directly.

- [ ] `sift read --outline <file.swift>` — Swift signature extraction (types, method signatures, conformances — no bodies)
- [ ] `sift find` improvements — auto-exclude DerivedData, Pods, `.build`, `xcuserdata`; group by directory; show exclusion summary
- [ ] `sift project` — full project snapshot: targets, bundle IDs, min iOS, Swift version, dependencies (CocoaPods/SPM/Carthage), source file counts, build configurations
- [ ] `sift crashlog <file>` — parse `.crash` and `.ips` crash reports: crash type, crashed thread backtrace (Swift-demangled), device/OS info (~95% reduction)
- [ ] `sift periphery` — dead code scan results grouped by file and symbol type (class, func, var, protocol)

---

## v0.9.0 — Project Introspection 🔮 Planned

Configuration and signing file parsing — the files AI agents read but no human can parse at a glance.

- [ ] `sift pbxproj <project.pbxproj>` — targets, bundle IDs, signing config, build phases, inter-target dependencies
- [ ] `sift plutil <file.plist>` — Info.plist and entitlements compact view: identity, privacy permissions, capabilities
- [ ] `sift provisioning <file.mobileprovision>` — profile type, app ID, team, expiry status, entitlements
- [ ] `sift xccov <file.xcresult>` — code coverage summary: overall %, files below threshold, uncovered functions
- [ ] `sift gh run view` / `sift gh run list` — GitHub Actions log filtering for iOS CI (strips timestamps, runner noise; reuses xcodebuild filters)
- [ ] `sift xclogparser <file.xcactivitylog>` — Xcode build activity log: errors, warnings, build phase times, slowest files to compile

---

## v1.0.0 — Stable Release 🔮 Planned

First stable release with full documentation, compatibility guarantees, and broad distribution.

- [ ] Stable public API contract for `sift-lib`
- [ ] Full documentation coverage
- [ ] Homebrew tap (`brew tap npescador/sift && brew install sift`)
- [ ] GitHub Actions CI release workflow — cross-compiled binaries per architecture (aarch64-apple-darwin, x86_64-apple-darwin, Linux)
- [ ] Windows support
- [ ] Performance benchmarks and regression tests

---

## Non-Goals (V1)

- GUI or web UI
- Remote or cloud execution
- Cloud telemetry (all tracking is always local)
- AI-based summarization (Sift uses rule-based filters by design for predictability)



