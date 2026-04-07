# Architecture

## Overview

Sift is a command proxy pipeline. Raw command output flows in, a compact high-signal summary flows out, and the original exit code is always preserved.

```
CLI Input
    │
    ▼
CommandDetector          ← identifies the command family from argv
    │
    ├── Known family ──▶ Executor ──▶ Filter ──▶ Tee check ──▶ Output
    │                                   │
    │                             (per-family logic)
    │
    └── Unknown ────────▶ Executor ──▶ Raw passthrough ──▶ Output

Exit code: always propagated unchanged from Executor
```

---

## Design Principles

1. **Exit code fidelity** — the exit code from the underlying command is always propagated unchanged. This is non-negotiable.
2. **Opt-in reduction** — when signal ambiguity exists, preserve more output, not less. Sift should never silently discard a compiler error.
3. **Safe passthrough** — unrecognized commands run natively with zero modification to their output.
4. **Modular filters** — each command family owns its own filter module with isolated logic and tests.
5. **No magic** — no silent failures, no hidden transformations, no output modification without the user's awareness.
6. **Raw escape hatch** — `--raw` always produces identical output to running the command directly.

---

## Module Structure

```
src/
├── main.rs              # Binary entry point, error boundary, filter dispatch (apply_filter)
├── cli.rs               # Argument parsing and CLI structure (clap)
├── executor.rs          # Subprocess spawn, output capture, exit code
├── config.rs            # Config file loading and defaults
├── tracking.rs          # SQLite-backed metrics and savings (stats.db)
├── init.rs              # sift init — shell hooks, CLAUDE.md, copilot instructions
├── tee.rs               # Tee mode — save raw to disk on empty filter result
├── error.rs             # Shared error types
├── commands/            # Command family detection and routing
│   ├── mod.rs           # CommandFamily enum, detect(), name()
│   ├── git.rs           # git subcommands (status, diff, log, log --graph)
│   ├── grep.rs          # grep / rg
│   ├── read.rs          # cat / head / tail / less
│   ├── swift_package.rs # swift package (resolve, update, show-dependencies)
│   ├── swift_build.rs   # swift build / swift test
│   ├── xcodebuild.rs    # xcodebuild subcommands (build, test, archive, -list, -showBuildSettings)
│   ├── xcrun.rs         # xcrun subcommands (simctl list, boot, install, launch, erase, delete)
│   ├── curl.rs          # curl
│   ├── pod.rs           # pod install / pod update
│   └── tuist.rs         # tuist (generate, fetch, cache, edit)
└── filters/             # Per-family output transformation
    ├── mod.rs           # FilterOutput type, Verbosity enum
    ├── git_status.rs    # git status → grouped file state summary
    ├── git_diff.rs      # git diff → per-file stats + hunk headers
    ├── git_log.rs       # git log / git log --graph → compact one-liners
    ├── grep.rs          # grep/rg → grouped by file, deduplicated
    ├── read.rs          # cat/read → truncated with line range support
    ├── ls_xcode.rs      # ls/find → Xcode-relevant files only
    ├── swift_package.rs # swift package → one line per dependency
    ├── swift_build.rs   # swift build → grouped errors, BUILD result
    ├── swift_test.rs    # swift test → pass/fail counts, failed assertions
    ├── swiftlint.rs     # swiftlint → violations grouped by rule
    ├── swiftformat.rs   # swiftformat → changed files, result summary
    ├── fastlane.rs      # fastlane → lane name, steps, result + time
    ├── xcodebuild_build.rs     # xcodebuild build → compiler + linker + signing errors
    ├── xcodebuild_test.rs      # xcodebuild test → pass/fail + failed test details
    ├── xcodebuild_archive.rs   # xcodebuild archive → result, signing, path
    ├── xcodebuild_list.rs      # xcodebuild -list → schemes, configs, targets
    ├── xcodebuild_settings.rs  # xcodebuild -showBuildSettings → 16 high-signal keys
    ├── xcrun_simctl.rs  # xcrun simctl list + boot/install/launch/erase/delete
    ├── codesign.rs      # codesign + security find-identity
    ├── agvtool.rs       # agvtool what-version / new-version / bump-versions
    ├── xcode_select.rs  # xcode-select --version / --print-path
    ├── curl.rs          # curl → HTTP status, key headers, truncated body
    ├── pod.rs           # pod install/update → per-pod summary
    ├── tuist.rs         # tuist generate/fetch/cache → targets, deps, errors
    ├── xcresulttool.rs  # xcresulttool → test summary from .xcresult bundles
    └── docc.rs          # docc → symbols processed, warnings, output path
```

---

## Key Types

### `CommandFamily`
```rust
pub enum CommandFamily {
    Git(GitSubcommand),
    Grep,
    Read,
    Ls,
    Find,
    Curl,
    Xcodebuild(XcodebuildSubcommand),
    Xcrun(XcrunSubcommand),
    Swiftlint,
    SwiftFormat,
    Fastlane,
    SwiftPackage(SwiftPackageSubcommand),
    SwiftBuild(SwiftBuildSubcommand),
    Pod(PodSubcommand),
    Tuist(TuistSubcommand),
    Codesign,
    Security,
    Agvtool,
    XcodeSelect,
    XcResultTool,
    DocC,
    /// Command not recognized — passed through unmodified.
    Unknown,
}
```
Detected from the first argument(s) of the user's command via `commands::detect(args)`.

### `ExecutorOutput`
```rust
pub struct ExecutorOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```
The raw result from running the underlying command. Never modified.

### `FilterOutput`
```rust
pub struct FilterOutput {
    pub content: String,       // Filtered output to print to stdout
    pub original_bytes: usize, // Size of the raw input
    pub filtered_bytes: usize, // Size of the filtered output
}
```
Produced by a filter from raw stdout. The filter selects content based on `Verbosity`.

### `Verbosity`
```rust
pub enum Verbosity {
    Compact,      // default — maximum signal reduction
    Verbose,      // -v
    VeryVerbose,  // -vv
    Maximum,      // -vvv
    Raw,          // --raw (bypasses filter entirely)
}
```

### `Config`
```rust
pub struct Config {
    pub defaults: DefaultsConfig,   // verbosity, max_lines
    pub tracking: TrackingConfig,   // enabled
    pub tee: TeeConfig,             // enabled
}
```
Loaded from `~/.config/sift/config.toml`. Missing file or fields use built-in defaults.

---

## Data Flow

### Normal filtered flow

1. User runs: `sift xcodebuild test -scheme MyApp`
2. `cli.rs` parses args → extracts command args and `Verbosity::Compact`
3. `commands::detect` → `CommandFamily::Xcodebuild(XcodebuildSubcommand::Test)`
4. `executor.rs` spawns `xcodebuild test -scheme MyApp`, captures stdout/stderr/exit code
5. `apply_filter` routes to `filters::xcodebuild_test::filter(&stdout, verbosity)`
6. `FilterOutput { content, original_bytes, filtered_bytes }` returned
7. **Tee check**: if `content` is empty but `stdout` was non-empty, fall back to raw and optionally save to `~/.local/share/sift/raw/<timestamp>-<cmd>.txt`
8. Sift writes `content` to stdout; stderr is always forwarded unchanged
9. `tracking::StatsFile::append(record)` persists metrics to SQLite
10. Sift exits with `output.exit_code`

### Passthrough flow

1. User runs: `sift some-unknown-tool --flag`
2. `commands::detect` returns `CommandFamily::Unknown`
3. `executor.rs` spawns the command, output is forwarded unchanged
4. Sift exits with the original exit code, no filtering applied

### Raw mode

1. User runs: `sift --raw git status`
2. `Verbosity::Raw` bypasses filter routing entirely
3. `executor.rs` output is forwarded directly to stdout/stderr
4. Sift exits with the original exit code

---

## Error Handling

- **Sift errors** (config parse failure, binary not found, etc.) exit with code `1` and print `[sift error] <message>` to stderr
- **Wrapped command failures** propagate the original exit code; the failed output is shown (filtered or raw, per mode)
- The distinction is always clear: `[sift error]` prefix is reserved for Sift's own failures

---

## Configuration

File: `~/.config/sift/config.toml` (`$XDG_CONFIG_HOME/sift/config.toml` if set)

- Loaded at startup via `config::load()`
- If absent, `Config::default()` applies — no file is required
- All fields are optional — missing fields fall back to defaults

---

## Tracking

`tracking::TrackingRecord` captures per-invocation metrics:
- command family name
- original byte count
- filtered byte count
- exit code
- duration (ms)
- Unix timestamp

Records are persisted to `~/.local/share/sift/stats.db` (SQLite via `rusqlite`). On first run, if a legacy `stats.toml` exists it is migrated automatically and renamed to `stats.toml.bak`.

---

## Dependencies

| Crate        | Purpose                                                |
|---|---|
| `clap`       | CLI argument parsing (derive API)                      |
| `serde`      | Config and tracking record deserialization             |
| `toml`       | Config file format; legacy stats migration             |
| `thiserror`  | Ergonomic error type derivation                        |
| `anyhow`     | Application-level error propagation in `main.rs`       |
| `rusqlite`   | SQLite persistence for tracking records (`stats.db`)   |
| `serde_json` | JSON export for `sift stats --json`                    |


## Overview

Sift is a command proxy pipeline. Raw command output flows in, a compact high-signal summary flows out, and the original exit code is always preserved.

```
CLI Input
    │
    ▼
CommandDetector          ← identifies the command family from argv
    │
    ├── Known family ──▶ Executor ──▶ Filter ──▶ Output
    │                                   │
    │                             (per-family logic)
    │
    └── Unknown ────────▶ Executor ──▶ Raw passthrough ──▶ Output

Exit code: always propagated unchanged from Executor
```

---

## Design Principles

1. **Exit code fidelity** — the exit code from the underlying command is always propagated unchanged. This is non-negotiable.
2. **Opt-in reduction** — when signal ambiguity exists, preserve more output, not less. Sift should never silently discard a compiler error.
3. **Safe passthrough** — unrecognized commands run natively with zero modification to their output.
4. **Modular filters** — each command family owns its own filter module with isolated logic and tests.
5. **No magic** — no silent failures, no hidden transformations, no output modification without the user's awareness.
6. **Raw escape hatch** — `--raw` always produces identical output to running the command directly.

---

## Module Structure

```
src/
├── main.rs              # Binary entry point, error boundary
├── cli.rs               # Argument parsing and CLI structure (clap)
├── executor.rs          # Subprocess spawn, output capture, exit code
├── config.rs            # Config file loading and defaults
├── tracking.rs          # Session metrics and savings calculation
├── commands/            # Command family detection and routing
│   ├── mod.rs           # CommandFamily enum, detection logic
│   ├── git.rs           # Git subcommand detection (status, diff, log…)
│   ├── grep.rs          # grep / rg detection
│   ├── read.rs          # cat / read detection
│   └── xcodebuild.rs    # xcodebuild subcommand detection (build, test…)
└── filters/             # Per-family output transformation
    ├── mod.rs           # FilterOutput type, Verbosity enum, routing
    ├── git_status.rs    # git status → grouped file state summary
    ├── git_diff.rs      # git diff → per-file stats + hunk headers
    ├── grep.rs          # grep/rg → grouped by file, deduplicated
    ├── read.rs          # cat/read → truncated with line range support
    ├── xcodebuild_build.rs  # xcodebuild build → grouped errors + warning counts
    └── xcodebuild_test.rs   # xcodebuild test → pass/fail + failed test details
```

---

## Key Types

### `CommandFamily`
```rust
pub enum CommandFamily {
    Git(GitSubcommand),
    Grep,
    Read,
    Xcodebuild(XcodebuildSubcommand),
    Unknown,
}
```
Detected from the first argument(s) of the user's command.

### `ExecutorOutput`
```rust
pub struct ExecutorOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u64,
}
```
The raw result from running the underlying command. Never modified.

### `FilterOutput`
```rust
pub struct FilterOutput {
    pub compact: String,       // Default verbosity output
    pub verbose: String,       // -v output
    pub very_verbose: String,  // -vv output
    pub original_bytes: usize,
    pub filtered_bytes: usize,
}
```
Produced by a filter from an `ExecutorOutput`. The `compact` field is printed by default.

### `Verbosity`
```rust
pub enum Verbosity {
    Compact,      // default
    Verbose,      // -v
    VeryVerbose,  // -vv
    Maximum,      // -vvv
    Raw,          // --raw (bypasses filter entirely)
}
```

### `Config`
```rust
pub struct Config {
    pub defaults: DefaultsConfig,
    pub tracking: TrackingConfig,
    pub commands: CommandsConfig,
}
```
Loaded from `~/.config/sift/config.toml`. If the file is absent, struct defaults apply. Missing fields use defaults — partial configs are always valid.

---

## Data Flow

### Normal filtered flow

1. User runs: `sift git status`
2. `cli.rs` parses args → extracts `["git", "status"]`, `Verbosity::Compact`
3. `commands::git` detects `CommandFamily::Git(GitSubcommand::Status)`
4. `executor.rs` spawns `git status`, captures stdout/stderr/exit code
5. `filters::git_status::filter(&output.stdout, verbosity)` produces `FilterOutput`
6. Sift writes `filter_output.compact` to stdout
7. `tracking.rs` records original vs filtered byte counts
8. Sift exits with `output.exit_code`

### Passthrough flow

1. User runs: `sift some-unknown-tool --flag`
2. `commands` returns `CommandFamily::Unknown`
3. `executor.rs` spawns `some-unknown-tool --flag`, streams output directly
4. Sift exits with the original exit code, no filtering applied

### Raw mode

1. User runs: `sift --raw git status`
2. `Verbosity::Raw` bypasses filter routing entirely
3. `executor.rs` output is streamed directly to stdout/stderr
4. Sift exits with the original exit code

---

## Error Handling

- **Sift errors** (config parse failure, binary not found, etc.) exit with code `1` and print `[sift error] <message>` to stderr
- **Wrapped command failures** propagate the original exit code; the failed output is shown (filtered or raw, per mode)
- The distinction is always clear: `[sift error]` prefix is reserved for Sift's own failures

---

## Configuration

File: `~/.config/sift/config.toml`

- Loaded at startup via `config::load()`
- If absent, `Config::default()` is used (no file is required)
- All fields are optional in the TOML — missing fields fall back to defaults
- Per-command overrides live under `[commands.<family>]` tables

---

## Tracking

`tracking::TrackingRecord` captures per-invocation metrics:
- command family
- original byte count
- filtered byte count
- timestamp (UTC)
- exit code

For the MVP, records are accumulated in-memory per session and reported via `sift stats`. SQLite persistence is planned for Milestone 4.

---

## Dependencies (Planned)

| Crate        | Justification                                          |
|--------------|--------------------------------------------------------|
| `clap`       | Industry-standard CLI argument parsing                 |
| `serde`      | Config deserialization; JSON output (future)           |
| `toml`       | Config file format (human-editable, widely understood) |
| `thiserror`  | Ergonomic error type derivation for library code       |
| `anyhow`     | Application-level error propagation in `main.rs`       |
| `rusqlite`   | SQLite tracking persistence (Milestone 4 only)         |

No dependency is added without a specific, documented need. `regex` is deferred until a filter requires it and simpler string matching proves insufficient.
