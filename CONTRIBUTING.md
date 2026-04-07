# Contributing to Sift

Thank you for your interest in contributing to Sift!

---

## Development Setup

### Requirements

- [Rust stable](https://rustup.rs) (1.75+)
- macOS (primary development platform; other platforms may work but are untested)
- `git`
- `cargo-edit` (optional, for dependency management): `cargo install cargo-edit`

### Getting Started

```bash
git clone https://github.com/npescador/sift
cd sift
git checkout develop    # develop is the default integration branch
cargo build
cargo test
```

### Running the CLI in Development

```bash
# Use cargo run during development
cargo run -- git status
cargo run -- --raw git status
cargo run -- -v git diff

# Or build and run the binary directly
cargo build --release
./target/release/sift git status
```

---

## Project Structure

See [ARCHITECTURE.md](ARCHITECTURE.md) for a detailed breakdown of the module structure and data flow.

---

## How to Contribute

1. **Check existing issues** before opening a new one — the feature or bug may already be tracked
2. **Open an issue first** for significant changes — discuss the approach before investing implementation time
3. **Fork the repository** and create a branch from `develop`
4. **Write tests** for any new behavior (filters, parsing, etc.)
5. **Run the full check suite** before submitting (see below)
6. **Submit a pull request** with a clear description of what changed and why

---

## Branch Strategy

Sift uses a simplified Git Flow model:

```
main        → production-ready code, official releases only
  └── develop   → integration branch (default) ← work happens here
        ├── feature/*    → new functionality
        ├── fix/*        → bug fixes
        ├── docs/*       → documentation only
        ├── refactor/*   → code restructuring
        ├── test/*       → test additions or fixes
        └── chore/*      → tooling, CI, dependencies

hotfix/*    → branches from main, for production emergencies
release/*   → branches from develop, for release preparation
```

**Normal workflow:**
1. Create `feature/my-feature` from `develop`
2. Open PR targeting `develop`
3. Once `develop` is stable, open PR from `develop` → `main` for a release

## Branch Naming

```
feature/short-description      # New features
fix/short-description          # Bug fixes
docs/short-description         # Documentation only
refactor/short-description     # Code restructuring without behavior change
test/short-description         # Test additions or fixes
chore/short-description        # Tooling, CI, dependency updates
hotfix/short-description       # Emergency fix from main
release/v0.x.0                 # Release preparation from develop
```

---

## Commit Convention

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add git status filter with grouped file state output
fix: preserve exit code in raw passthrough mode
docs: update architecture overview with executor data flow
test: add unit tests for xcodebuild error grouping
chore: update clap to 4.5
refactor: extract filter trait to shared module
perf: reduce allocation in grep filter hot path
```

Breaking changes must include `BREAKING CHANGE:` in the commit footer:

```
feat: redesign filter API for composability

BREAKING CHANGE: FilterOutput::lines is now FilterOutput::compact_lines.
Callers must update to the new field name.
```

---

## Pre-Submit Checklist

```bash
cargo test                              # All tests pass
cargo clippy -- -D warnings            # Zero clippy warnings
cargo fmt --all -- --check             # Code is formatted
```

All three must pass before a PR will be reviewed.

---

## Code Style

- Run `cargo fmt` before committing — formatting is non-negotiable
- Fix all `cargo clippy` warnings — do not suppress them with `#[allow(...)]` without a documented reason
- No `unwrap()` or `expect()` in production paths without an explicit comment explaining why it is safe
- No `#[allow(dead_code)]` in production paths — remove unused code instead
- Use `thiserror`-derived error types for library-facing errors
- Prefer `anyhow` for application-level error propagation in `main.rs`

---

## Adding a New Command Filter

1. Add the command family to `CommandFamily` enum in `src/commands/mod.rs` and update `detect()` and `name()`
2. If the command has subcommands, create `src/commands/<family>.rs` with a `detect_subcommand` function
3. Create `src/filters/<family>.rs` with a `filter(raw: &str, verbosity: Verbosity) -> FilterOutput` function
4. Declare the module in `src/filters/mod.rs` (`pub mod <family>;`)
5. Add a dispatch arm in `apply_filter()` in `src/main.rs`
6. Add unit tests in the filter file using inline test fixtures
7. Update the command table in `README.md`

---

## Reporting Bugs

Use the [bug report template](.github/ISSUE_TEMPLATE/bug_report.md).

Always include:
- `sift --version` output
- Operating system and version
- The exact command you ran
- Expected vs. actual output
- Any relevant error messages

---

## Feature Requests

Use the [feature request template](.github/ISSUE_TEMPLATE/feature_request.md).

---

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
