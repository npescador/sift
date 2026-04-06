# Sift

> Smart output reduction for AI-assisted developer workflows.

[![CI](https://github.com/ipescador/sift/actions/workflows/ci.yml/badge.svg)](https://github.com/ipescador/sift/actions/workflows/ci.yml)
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
# Instead of this (potentially thousands of lines):
xcodebuild test -scheme MyApp

# Use this (compact, agent-readable summary):
sift xcodebuild test -scheme MyApp
```

Sift intercepts the command, runs it natively, captures all output, and returns a filtered summary. **The original exit code is always preserved.**

---

## Quick Start

### Install from source

```bash
git clone https://github.com/ipescador/sift
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
```

---

## Supported Commands

| Command Family      | Status   | Description                                    |
|---------------------|----------|------------------------------------------------|
| `git status`        | ✅ MVP   | Grouped file state summary with counts         |
| `git diff`          | ✅ MVP   | Per-file stats, useful hunk headers only       |
| `grep` / `rg`       | ✅ MVP   | Results grouped by file, deduplication         |
| `cat` / `read`      | ✅ MVP   | Safe truncation, configurable line ranges      |
| `xcodebuild build`  | ✅ MVP   | Grouped unique errors, warning count summary   |
| `xcodebuild test`   | ✅ MVP   | Pass/fail counts, failed test details          |

Unknown commands pass through **unmodified**.

---

## Verbosity Modes

```bash
sift git diff              # Compact (default) — maximum signal reduction
sift -v git diff           # Verbose — more context retained
sift -vv git diff          # Very verbose — near-complete output
sift -vvv git diff         # Maximum — minimal filtering
sift --raw git diff        # Raw passthrough — zero filtering
```

---

## Configuration

Sift reads `~/.config/sift/config.toml` (created on first run if absent):

```toml
[defaults]
verbosity = "compact"
max_lines = 100

[tracking]
enabled = true

[commands.xcodebuild]
max_errors = 20
max_warnings = 10
```

---

## Tracking & Savings

```bash
sift stats                 # Show token/line savings for current session
sift stats --all           # Show historical totals
```

---

## Why Not Just Pipe to `head`?

`head` blindly truncates. Sift understands command output structure — it knows the difference between a compiler error and a build progress line, between a relevant git hunk header and boilerplate diff metadata. That understanding is what makes the output useful for AI agents, not just shorter.

---

## Status

🚧 **Early Development** — MVP in active development. See [ROADMAP.md](ROADMAP.md) for current milestones.

---

## Documentation

- [Architecture](ARCHITECTURE.md) — Design overview and module breakdown
- [Roadmap](ROADMAP.md) — Planned features and milestones
- [Contributing](CONTRIBUTING.md) — How to contribute
- [Changelog](CHANGELOG.md) — Version history

---

## License

MIT — see [LICENSE](LICENSE).
