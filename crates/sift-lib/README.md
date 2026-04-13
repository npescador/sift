# sift-lib

Programmatic library API for [Sift](https://github.com/npescador/sift) — a smart output reduction layer for AI coding workflows.

Sift filters verbose shell command output down to high-signal summaries, reducing the token cost of feeding terminal output to AI coding agents (GitHub Copilot CLI, Claude Code, Codex CLI, etc.).

## Installation

```toml
[dependencies]
sift-lib = "0.6"
```

## Usage

### Filter pre-captured output

Use `filter` when you already have the raw output and just need the compact summary:

```rust
use sift_lib::{filter, Verbosity};

let raw = std::fs::read_to_string("build.log").unwrap();
let out = filter(&["xcodebuild", "build"], &raw, Verbosity::Compact);

println!("{}", out.content);
// BUILD FAILED
//
// src/PaymentService.swift (2 errors)
//   error: use of unresolved identifier 'PaymentResult'
//   error: cannot convert value of type 'String' to expected argument type 'Amount'
```

### Execute and filter in one call

Use `run` to spawn the command and get filtered output in a single step:

```rust
use sift_lib::{run, Verbosity};

let result = run(&["git", "diff"], Verbosity::Compact)?;
println!("{}", result.filtered.content);
// src/executor.rs       +12  -3
// src/filters/mod.rs     +5  -1
// ─────────────────────
// 2 files changed, +17 -4
assert_eq!(result.exit_code, 0);
```

### Detect command family

```rust
use sift_lib::{detect_family, CommandFamily};

let family = detect_family(&["xcodebuild", "test", "-scheme", "MyApp"]);
assert!(matches!(family, CommandFamily::Xcodebuild(_)));
```

## Verbosity levels

| Value | CLI flag | Description |
|---|---|---|
| `Verbosity::Compact` | _(default)_ | Maximum reduction — best for AI agents |
| `Verbosity::Verbose` | `-v` | More context retained |
| `Verbosity::VeryVerbose` | `-vv` | Near-complete output |
| `Verbosity::Maximum` | `-vvv` | Minimal filtering |
| `Verbosity::Raw` | `--raw` | No filtering — identical to direct invocation |

## Supported command families

All 27 command families from the Sift CLI are supported:
`git`, `grep`/`rg`, `cat`/`head`/`tail`, `ls`, `find`, `curl`,
`xcodebuild`, `xcrun`, `xcresulttool`, `docc`,
`swiftlint`, `swiftformat`, `fastlane`,
`swift build`/`swift test`, `swift package`,
`pod`, `tuist`, `codesign`, `security`, `agvtool`,
`xcode-select`, and `Unknown` (passthrough).

## License

MIT — see [LICENSE](../../LICENSE).
