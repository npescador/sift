# Sift

> Smart output reduction for AI-assisted developer workflows.

[![CI](https://github.com/npescador/sift/actions/workflows/ci.yml/badge.svg)](https://github.com/npescador/sift/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange.svg)](https://www.rust-lang.org)

Sift is a Rust-based command proxy that sits between your shell and your tools. It captures raw command output, applies smart filtering, and returns a compact, high-signal summary — without losing exit codes, errors, or critical diagnostics.

Built for AI-assisted coding workflows where token efficiency and signal clarity matter.

---

## The Problem

AI coding agents (Copilot CLI, Codex CLI, Claude Code) consume terminal output as context. Commands like `xcodebuild test`, `git diff`, and `rg` regularly produce thousands of lines that:

- waste tokens and context window space
- dilute the signal agents need to reason about
- slow down iteration cycles
- hit context limits on complex tasks

## The Solution

```bash
# Without Sift — xcodebuild test (thousands of lines):
Test Suite 'All tests' started at 2026-04-06 10:00:00.000
Test Case '-[LoginTests testValidUser]' started
...
** TEST FAILED **

# With Sift — compact summary:
TEST FAILED  47 tests — 45 passed, 2 failed

  ✗ -[PaymentTests testCheckout]
    XCTAssertEqual failed: ("200") is not equal to ("404")

  ✗ -[AuthTests testTokenRefresh]
    XCTAssertNotNil failed
```

Sift intercepts the command, runs it natively, captures all output, and returns a filtered summary. **The original exit code is always preserved.**

---

## Quick Start

### Install from source

```bash
git clone https://github.com/npescador/sift
cd sift
cargo build --release
cp target/release/sift /usr/local/bin/sift
```

### Basic usage

```bash
sift git status
sift git diff HEAD~1
sift rg "fn main" src/
sift xcodebuild test -scheme MyApp -destination "platform=iOS Simulator,name=iPhone 16"
sift swift build
sift pod install
sift stats
```

### Shell integration (recommended)

Run once to inject automatic shell hooks so every supported command is filtered transparently:

```bash
sift init --shell          # wraps git, xcodebuild, xcrun, swiftlint, pod… in ~/.zshrc
sift init --claude         # creates/updates CLAUDE.md for Claude Code
sift init --copilot        # creates/updates .github/copilot-instructions.md
sift init --show           # show current installation status
sift init --uninstall      # remove all sift-managed hooks and instruction files
```

After `sift init --shell`, commands like `git diff` automatically go through Sift — no prefix needed.

---

## Supported Commands

### Version Control

| Command | Description |
|---|---|
| `git status` | Grouped file state summary with counts |
| `git diff` | Per-file `+N -N` stats, useful hunk headers |
| `git log` | One line per commit: hash · subject · date · author |
| `git log --graph` | Graph decoration stripped, compact log format preserved |

### iOS Build & Test

| Command | Description |
|---|---|
| `xcodebuild build` | Errors grouped by file (compiler + linker 🔗 + signing 🔐), warning count |
| `xcodebuild test` | Pass/fail/skip counts, failed test names and XCTAssert messages |
| `xcodebuild archive` | Archive result, scheme/config, path, signing identity |
| `xcodebuild -list` | Default scheme ★, configurations, target count |
| `xcodebuild -showBuildSettings` | 16 high-signal iOS keys from ~400-line output |
| `swift build` | Errors grouped by file, `BUILD SUCCEEDED/FAILED` |
| `swift test` | Pass/fail counts, failed test names and assertions |
| `xcresulttool` | Test summary from `.xcresult` bundles (CI-friendly) |

### Swift Toolchain

| Command | Description |
|---|---|
| `swift package resolve/update` | One line per package: name + version |
| `swift package show-dependencies` | Dependency tree, compact |
| `swiftlint` | Violations grouped by rule, errors before warnings |
| `swiftformat` | Files changed, result summary, lint errors |
| `docc convert/preview` | Symbols processed, warnings, output path |

### Dependencies & Project Generation

| Command | Description |
|---|---|
| `pod install` / `pod update` | One pod per line, warnings, install result |
| `tuist generate/fetch/cache` | Targets generated, dependencies resolved, errors |
| `fastlane` | Lane name, step progression, result + total time |

### Signing & Distribution

| Command | Description |
|---|---|
| `codesign` | Signing status, identifier, team, format |
| `security find-identity` | Valid identities with short hash and name |
| `agvtool` | Current/new version, files updated |
| `xcode-select` | Active Xcode version and path |

### Simulator

| Command | Description |
|---|---|
| `xcrun simctl list` | Booted first, short UDID, iOS-only compact view |
| `xcrun simctl boot/install/launch/erase/delete` | Compact operation result |

### Search & File Utilities

| Command | Description |
|---|---|
| `grep` / `rg` | Results grouped by file, capped per file and total |
| `cat` / `head` / `tail` / `less` | Safe truncation, binary detection |
| `ls` / `find` | Xcode-relevant files only; drops `.build/`, `DerivedData/`, `.o` |
| `curl` | HTTP status, key headers, body truncated to N lines |

Unknown commands pass through **unmodified** with the original exit code.

---

## Verbosity Modes

```bash
sift git diff              # Compact (default) — maximum signal reduction
sift -v git diff           # Verbose — more context retained
sift -vv git diff          # Very verbose — near-complete output
sift -vvv git diff         # Maximum — minimal filtering
sift --raw git diff        # Raw passthrough — zero filtering
```

Default verbosity can be set in the config file.

---

## Configuration

Sift reads `~/.config/sift/config.toml`. All fields are optional.

```toml
[defaults]
verbosity = "compact"   # compact | verbose | very_verbose | maximum | raw
max_lines = 100         # default truncation limit for cat/read filter

[tracking]
enabled = true          # set false to disable stats recording

[tee]
enabled = true          # save raw output to disk when filter produces empty result
```

**Config resolution:**
1. `$XDG_CONFIG_HOME/sift/config.toml` if `XDG_CONFIG_HOME` is set
2. `~/.config/sift/config.toml` otherwise

**Verbosity priority:** `--raw` > `-v` flags > config default

### Tee mode

When a filter produces empty output from non-empty input (possible false negative), Sift falls back to raw output and saves a copy to `~/.local/share/sift/raw/<timestamp>-<cmd>.txt` with a warning on stderr. Disable with `[tee] enabled = false`.

---

## Tracking & Savings

Stats are persisted to `~/.local/share/sift/stats.db` (SQLite, `$XDG_DATA_HOME` aware). If a legacy `stats.toml` exists from an older version it is migrated automatically on first run.

```bash
sift stats               # show all historical savings
sift stats --last 20     # last 20 invocations only
sift stats --reset       # clear all history
sift stats --json        # export full history as JSON
```

```
Sift Statistics
─────────────────────────────────────────
  Invocations:    47
  Original bytes: 2.1 MB
  Filtered bytes: 98.3 KB
  Bytes saved:    2.0 MB  (95.4% avg)
─────────────────────────────────────────
  By command:
    git          23 runs
    xcodebuild   15 runs
    grep          9 runs
```

---

## AI Agent Integration

See **[AGENTS.md](AGENTS.md)** for setup guides for:
- GitHub Copilot CLI
- OpenAI Codex CLI
- Anthropic Claude Code

---

## Why Not Just Pipe to `head`?

`head` blindly truncates. Sift understands command output structure — it knows the difference between a compiler error and a build progress line, between a relevant git hunk header and boilerplate diff metadata. That understanding is what makes the output useful for AI agents, not just shorter.

---

## Status

**v0.4.0** — See [CHANGELOG.md](CHANGELOG.md) for the full version history and [ROADMAP.md](ROADMAP.md) for planned features.

---

## Documentation

- [AGENTS.md](AGENTS.md) — AI agent integration guide
- [ARCHITECTURE.md](ARCHITECTURE.md) — Design overview and module breakdown
- [ROADMAP.md](ROADMAP.md) — Planned features and milestones
- [CONTRIBUTING.md](CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](CHANGELOG.md) — Version history

---

## License


MIT — © 2026 Nacho Pescador Ruiz. See [LICENSE](LICENSE).
