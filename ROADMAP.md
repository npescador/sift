# Roadmap

This document tracks Sift's planned milestones. Each milestone represents a coherent set of capabilities that can be released together.

For granular progress, see the [GitHub Issues](https://github.com/ipescador/sift/issues) tracker.

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
