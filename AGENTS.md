# Sift — AI Agent Integration Guide

This guide explains how to configure Sift with the three primary AI coding agent CLIs: **GitHub Copilot CLI**, **OpenAI Codex CLI**, and **Anthropic Claude Code**.

Sift reduces the token cost of shell commands by returning compact, high-signal summaries instead of raw verbose output. Each agent gets the information it needs — without the noise.

---

## Why Agents Benefit from Sift

AI coding agents read terminal output as part of their context window. Without filtering:

| Command | Typical raw output | With Sift |
|---|---|---|
| `xcodebuild test` | 2,000–10,000 lines | ~10 lines |
| `git diff` (large PR) | 500–5,000 lines | ~20 lines |
| `rg "pattern" src/` | 100–1,000 lines | ~15 lines |
| `xcodebuild build` (errors) | 300–2,000 lines | ~10 lines |

Reducing output means:
- more context available for reasoning
- lower cost per agent invocation
- faster iteration on errors and diffs

---

## Setup

### 1. Install Sift

```bash
git clone https://github.com/npescador/sift
cd sift
cargo build --release
cp target/release/sift /usr/local/bin/sift
```

Verify:

```bash
sift --version
```

### 2. Configure (optional)

```bash
mkdir -p ~/.config/sift
cat > ~/.config/sift/config.toml << 'EOF'
[defaults]
verbosity = "compact"

[tracking]
enabled = true
EOF
```

### 3. Verify stats tracking

```bash
sift git status        # run any command
sift stats             # confirm recording works
```

---

## GitHub Copilot CLI

Copilot CLI (`gh copilot suggest`, `gh copilot explain`) reads shell output when you run commands in its session context.

### Shell configuration

Add to `~/.zshrc` (or `~/.bashrc`):

```bash
# Sift aliases for Copilot CLI sessions
alias git='sift git'
alias rg='sift rg'
alias grep='sift grep'
alias xcodebuild='sift xcodebuild'
```

### Usage pattern

When Copilot CLI runs a command to understand your codebase, it automatically goes through Sift:

```bash
# Copilot asks you to run: git diff HEAD~1
# With the alias, it becomes: sift git diff HEAD~1
# Output returned to Copilot: compact per-file stats only
```

### Raw passthrough when needed

If Copilot needs full output for a specific command:

```bash
\git diff HEAD~1         # backslash bypasses the alias
sift --raw git diff      # explicit raw flag
```

---

## OpenAI Codex CLI

Codex CLI executes shell commands to gather context and verify changes. Sift wraps those commands transparently.

### Shell configuration

```bash
# Add to ~/.zshrc
alias git='sift git'
alias rg='sift rg'
alias xcodebuild='sift xcodebuild'
alias cat='sift cat'
```

### Explicit prefix pattern

Alternatively, configure Codex to prefix commands with `sift`:

```bash
# In your Codex system prompt or project instructions:
# "Prefix all shell commands with 'sift' to receive compact output."
sift git status
sift rg "TODO" src/
sift xcodebuild build -scheme MyApp
```

### Exit code behaviour

Codex uses exit codes to determine whether commands succeeded. Sift always propagates the exact exit code from the underlying command — a failed build with `xcodebuild` still returns exit code `65`, even through Sift.

---

## Anthropic Claude Code

Claude Code (`claude`) runs shell commands as part of its agentic loop. Sift is particularly effective here because Claude reads full command output as context tokens.

### Recommended approach: CLAUDE.md

Create a `CLAUDE.md` at your project root to instruct Claude to use Sift:

```markdown
## Shell Commands

Always prefix shell commands with `sift` to receive compact output:

- `sift git status` instead of `git status`
- `sift git diff` instead of `git diff`
- `sift rg <pattern> <path>` instead of `rg <pattern> <path>`
- `sift xcodebuild build -scheme <scheme>` instead of `xcodebuild build`
- `sift xcodebuild test -scheme <scheme>` instead of `xcodebuild test`

Use `sift --raw <command>` only when you need the full, unfiltered output.
Use `sift stats` to check accumulated token savings for this session.
```

### Verbosity guidance for Claude

```markdown
## Sift Verbosity

- Default (no flag): maximum reduction, best for initial checks
- `-v`: more detail when debugging a specific error
- `--raw`: full output when you need to inspect exact content
```

### Example Claude Code session

```
Claude: Let me check the test results.
$ sift xcodebuild test -scheme MyApp -destination "platform=iOS Simulator,name=iPhone 16 Pro"

TEST FAILED  23 tests — 21 passed, 2 failed

  ✗ -[NetworkTests testTimeoutHandling]
    XCTAssertEqual failed: ("408") is not equal to ("200")

  ✗ -[CacheTests testInvalidation]
    XCTAssertNil failed

Claude: Two tests are failing. The timeout test expects 408 but gets 200...
```

Instead of Claude reading 2,000+ lines of `xcodebuild` output, it sees exactly what matters.

---

## Verbosity Reference

| Flag | Level | Use case |
|------|-------|----------|
| _(none)_ | Compact | Default — maximum reduction, best for agents |
| `-v` | Verbose | More detail when investigating a specific failure |
| `-vv` | Very Verbose | Near-complete output for deep debugging |
| `-vvv` | Maximum | Minimal filtering |
| `--raw` | Raw | Zero filtering — identical to direct invocation |

---

## Filter Behaviour Reference

### `sift git status`

```
On branch main
staged:    2 files
modified:  1 file
untracked: 4 files  (+2 more)
```

### `sift git diff`

```
src/executor.rs       +12  -3
src/filters/mod.rs     +5  -1
tests/integration.rs  +47  -0
─────────────────────────────
3 files changed, +64 -4
```

### `sift rg "pattern" src/`

```
src/main.rs (2 matches)
  42: fn apply_filter(args: &[String], ...
  87: match commands::detect(args) {

src/filters/mod.rs (1 match)
  10: pub enum Verbosity {
```

### `sift xcodebuild build`

```
BUILD FAILED

src/PaymentService.swift (2 errors)
  error: use of unresolved identifier 'PaymentResult'
  error: cannot convert value of type 'String' to expected argument type 'Amount'

src/NetworkClient.swift (1 error)
  error: value of type 'URLSession' has no member 'dataTaskAsync'

2 errors — 3 warnings
```

### `sift xcodebuild test`

```
TEST FAILED  47 tests — 45 passed, 2 failed

  ✗ -[PaymentTests testCheckout]
    XCTAssertEqual failed: ("200") is not equal to ("404")

  ✗ -[AuthTests testTokenRefresh]
    XCTAssertNotNil failed
```

---

## Tracking

```bash
sift stats
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

## Troubleshooting

**Output looks wrong or truncated unexpectedly**
→ Use `sift --raw <command>` to see the original output and compare.

**A command not in the supported list**
→ Sift passes it through unmodified. The exit code is preserved.

**Tracking not recording**
→ Check `~/.config/sift/config.toml` has `[tracking] enabled = true` (default).
→ Check write permissions on `~/.local/share/sift/`.

**Need full output for a one-off**
→ `sift --raw <command>` or `\<command>` (shell alias bypass).

---

## Contribution & Commit Guidelines

### Commit message format

One line only, in English, using Conventional Commits type prefix:

```
type: short description
```

Valid types: `feat`, `fix`, `refactor`, `test`, `chore`, `docs`, `ci`

Examples:
```
feat: add fastlane filter with ~85% reduction
fix: compact_date uses dynamic year instead of hardcoded value
chore: bump version to 0.3.0
```

**Rules:**
- Single line — no multi-line body in commit message
- No `Co-authored-by` trailers
- No attribution lines of any kind
- Lowercase description after the colon

---

## PR Creation Reference

When a feature branch is ready, provide this metadata to the developer:

| Field | Value |
|---|---|
| **Base branch** | `develop` (features) · `main` (releases only) |
| **Milestone** | Match the target version (see below) |
| **Labels** | See tables below |

### Milestones

| # | Title | Scope |
|---|---|---|
| 1 | `v0.1.0 — Foundation` | MVP core |
| 2 | `v0.2.0 — Xcode Support` | iOS/Xcode filters |
| 3 | `v0.3.0 — Shell Integration` | fastlane, swift package, archive, init improvements |
| 4 | `v1.0.0 — Stable Release` | Stable, full docs |

### Labels by type

| Label | When to use |
|---|---|
| `enhancement` | New filter or feature |
| `bug` | Fix to existing behaviour |
| `fix` | Fix (alternative) |
| `refactor` | Code restructure, no behaviour change |
| `test` | Test additions or fixes |
| `chore` | CI, tooling, version bumps |
| `documentation` | AGENTS.md, README, CHANGELOG only |
| `ci` | GitHub Actions changes |

### Labels by module

| Label | When to use |
|---|---|
| `mod: filters` | Changes in `src/filters/` |
| `mod: executor` | Changes in `src/executor.rs` |
| `mod: cli` | Changes in `src/cli.rs` |
| `mod: config` | Changes in `src/config.rs` |
| `mod: tracking` | Changes in `src/tracking.rs` |

### Labels by command family

| Label | When to use |
|---|---|
| `cmd: git` | git-related filters |
| `cmd: xcodebuild` | xcodebuild filters |
| `cmd: grep` | grep/rg filter |
| `cmd: read` | cat/head/tail filter |

> For a new command family not in the list (e.g. fastlane, swiftlint), use `enhancement` + `mod: filters`.
