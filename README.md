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
sift stats
```

---

## Supported Commands

| Command Family      | Status   | Description                                    |
|---------------------|----------|------------------------------------------------|
| `git status`        | ✅ v0.1  | Grouped file state summary with counts         |
| `git diff`          | ✅ v0.1  | Per-file stats, useful hunk headers only       |
| `grep` / `rg`       | ✅ v0.1  | Results grouped by file, capped per file       |
| `cat` / file reads  | ✅ v0.1  | Safe truncation, binary detection              |
| `xcodebuild build`  | ✅ v0.1  | Grouped unique errors, warning count summary   |
| `xcodebuild test`   | ✅ v0.1  | Pass/fail counts, failed test details          |

Unknown commands pass through **unmodified** with the original exit code.

---

## Verbosity Modes

```bash
sift git diff              # Compact (default) — maximum signal reduction
sift -v git diff           # Verbose — more context retained
sift -vv git diff          # Very verbose — near-complete output
sift -vvv git diff         # Maximum — minimal filtering
sift --raw git diff        # Raw passthrough — zero filtering, identical to direct invocation
```

Default verbosity can be set in the config file.

---

## Configuration

Sift reads `~/.config/sift/config.toml`. All fields are optional — missing file uses built-in defaults.

```toml
[defaults]
verbosity = "compact"   # compact | verbose | very_verbose | maximum | raw
max_lines = 100         # default truncation limit for cat/read filter

[tracking]
enabled = true          # set false to disable stats recording
```

**Config resolution:**
1. `$XDG_CONFIG_HOME/sift/config.toml` if `XDG_CONFIG_HOME` is set
2. `~/.config/sift/config.toml` otherwise

**Verbosity priority:** `--raw` > `-v` flags > config default

---

## Tracking & Savings

```bash
sift stats               # Show accumulated token savings
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

Stats are persisted to `~/.local/share/sift/stats.toml` (`$XDG_DATA_HOME/sift/stats.toml` if set).

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

**v0.1.0** — MVP complete. All core command filters implemented. See [ROADMAP.md](ROADMAP.md) for planned features.

---

## Documentation

- [AGENTS.md](AGENTS.md) — AI agent integration guide
- [ARCHITECTURE.md](ARCHITECTURE.md) — Design overview and module breakdown
- [ROADMAP.md](ROADMAP.md) — Planned features and milestones
- [CONTRIBUTING.md](CONTRIBUTING.md) — How to contribute
- [CHANGELOG.md](CHANGELOG.md) — Version history

---

## License

MIT — see [LICENSE](LICENSE).
