//! `sift init` — install shell hooks and AI agent instruction files.
//!
//! Shell hooks wrap commonly noisy commands so every invocation is
//! automatically filtered without the user typing `sift` explicitly.
//!
//! Marker-based injection makes the operation idempotent: running
//! `sift init --shell` twice produces the same result.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Marker written into shell rc files to identify the sift block.
const BLOCK_START: &str = "# BEGIN sift hooks";
const BLOCK_END: &str = "# END sift hooks";

/// Shell hook functions injected into the rc file.
///
/// Using `command sift` (instead of bare `sift`) prevents re-entrancy
/// if the user ever wraps sift itself in a shell function. Rust's
/// `std::process::Command` already bypasses shell functions, but being
/// explicit here is defensive and correct.
const SHELL_HOOK_BODY: &str = r#"
git()          { command sift git "$@"; }
xcodebuild()   { command sift xcodebuild "$@"; }
xcrun()        { command sift xcrun "$@"; }
swiftlint()    { command sift swiftlint "$@"; }
"#;

/// CLAUDE.md content injected / appended to the project file.
const CLAUDE_MD_BLOCK_START: &str = "<!-- BEGIN sift instructions -->";
const CLAUDE_MD_BLOCK_END: &str = "<!-- END sift instructions -->";

const CLAUDE_MD_CONTENT: &str = r#"## Sift — compact command output

Shell commands are automatically filtered by [sift](https://github.com/npescador/sift).
You will receive compact, high-signal output instead of raw verbose output.

**You MUST use sift for these commands:**
```
sift git status
sift git diff
sift git log
sift xcodebuild build   -scheme <Scheme> -destination <dest>
sift xcodebuild test    -scheme <Scheme> -destination <dest>
sift xcrun simctl list
sift xcodebuild -showBuildSettings
sift swiftlint lint
```

**Verbosity flags:**
- Default: compact (maximum token savings)
- `-v`   : verbose (adds file locations)
- `-vv`  : very verbose (near-complete output)
- `--raw`: raw passthrough (identical to running command directly)

**Token savings stats:**
```
sift stats
```

If a command output seems truncated, add `-v` to get more context."#;

/// .github/copilot-instructions.md content.
const COPILOT_BLOCK_START: &str = "<!-- BEGIN sift instructions -->";
const COPILOT_BLOCK_END: &str = "<!-- END sift instructions -->";

const COPILOT_CONTENT: &str = r#"## Sift shell hooks active

This project has [sift](https://github.com/npescador/sift) shell hooks installed.
All terminal commands are auto-filtered for compact output.

When running shell commands, prefer the sift-aware variants:
- `sift git status` / `sift git diff` / `sift git log`
- `sift xcodebuild build|test|archive`
- `sift xcrun simctl list`
- `sift xcodebuild -showBuildSettings`
- `sift swiftlint lint`

Use `--raw` to get unfiltered output when full detail is needed."#;

// ── Public API ──────────────────────────────────────────────────────────────

pub struct InitOptions {
    pub shell: bool,
    pub claude: bool,
    pub copilot: bool,
    pub show: bool,
    pub uninstall: bool,
}

/// Entry point called from `main.rs`.
pub fn run(opts: InitOptions) -> Result<()> {
    // Default: if no flag specified, treat as --show
    if !opts.shell && !opts.claude && !opts.copilot && !opts.uninstall {
        return show_status();
    }

    if opts.show {
        return show_status();
    }

    if opts.uninstall {
        return uninstall_all();
    }

    if opts.shell {
        install_shell_hook()?;
    }
    if opts.claude {
        install_claude_md()?;
    }
    if opts.copilot {
        install_copilot_instructions()?;
    }

    Ok(())
}

// ── Shell hook ───────────────────────────────────────────────────────────────

fn install_shell_hook() -> Result<()> {
    let rc_path = detect_rc_file()?;
    let current = fs::read_to_string(&rc_path)
        .unwrap_or_default();

    let block = build_shell_block();

    let new_content = if current.contains(BLOCK_START) {
        replace_block(&current, BLOCK_START, BLOCK_END, &block)
    } else {
        format!("{}\n{}\n", current.trim_end_matches('\n'), block)
    };

    fs::write(&rc_path, new_content)
        .with_context(|| format!("failed to write {}", rc_path.display()))?;

    println!(
        "✅ Shell hooks installed in {}",
        rc_path.display()
    );
    println!();
    println!("   Wrapped commands: git, xcodebuild, xcrun, swiftlint");
    println!();
    println!("   Reload your shell:");
    println!("     source {}", rc_path.display());

    Ok(())
}

fn build_shell_block() -> String {
    format!("{BLOCK_START}\n# Managed by `sift init --shell` — do not edit manually{SHELL_HOOK_BODY}{BLOCK_END}\n")
}

fn detect_rc_file() -> Result<PathBuf> {
    let home = home_dir()?;
    // Prefer zsh (default on macOS since Catalina), fall back to bash
    let shell = std::env::var("SHELL").unwrap_or_default();
    if shell.contains("zsh") {
        return Ok(home.join(".zshrc"));
    }
    if shell.contains("bash") {
        let zshrc = home.join(".zshrc");
        if zshrc.exists() {
            return Ok(zshrc);
        }
        return Ok(home.join(".bashrc"));
    }
    // Fallback: zshrc
    Ok(home.join(".zshrc"))
}

// ── CLAUDE.md ────────────────────────────────────────────────────────────────

fn install_claude_md() -> Result<()> {
    let path = PathBuf::from("CLAUDE.md");
    let current = fs::read_to_string(&path).unwrap_or_default();

    let block = format!(
        "{CLAUDE_MD_BLOCK_START}\n{CLAUDE_MD_CONTENT}\n{CLAUDE_MD_BLOCK_END}\n"
    );

    let new_content = if current.contains(CLAUDE_MD_BLOCK_START) {
        replace_block(&current, CLAUDE_MD_BLOCK_START, CLAUDE_MD_BLOCK_END, &block)
    } else if current.is_empty() {
        block
    } else {
        format!("{}\n\n{}", current.trim_end_matches('\n'), block)
    };

    fs::write(&path, new_content)
        .with_context(|| "failed to write CLAUDE.md")?;

    println!("✅ CLAUDE.md updated with sift instructions");
    Ok(())
}

// ── Copilot instructions ──────────────────────────────────────────────────────

fn install_copilot_instructions() -> Result<()> {
    let dir = Path::new(".github");
    if !dir.exists() {
        fs::create_dir_all(dir).with_context(|| "failed to create .github/")?;
    }

    let path = dir.join("copilot-instructions.md");
    let current = fs::read_to_string(&path).unwrap_or_default();

    let block = format!(
        "{COPILOT_BLOCK_START}\n{COPILOT_CONTENT}\n{COPILOT_BLOCK_END}\n"
    );

    let new_content = if current.contains(COPILOT_BLOCK_START) {
        replace_block(&current, COPILOT_BLOCK_START, COPILOT_BLOCK_END, &block)
    } else if current.is_empty() {
        block
    } else {
        format!("{}\n\n{}", current.trim_end_matches('\n'), block)
    };

    fs::write(&path, new_content)
        .with_context(|| "failed to write .github/copilot-instructions.md")?;

    println!("✅ .github/copilot-instructions.md updated with sift instructions");
    Ok(())
}

// ── Uninstall ────────────────────────────────────────────────────────────────

fn uninstall_all() -> Result<()> {
    let mut any = false;

    // Shell rc file
    if let Ok(rc_path) = detect_rc_file() {
        if let Ok(content) = fs::read_to_string(&rc_path) {
            if content.contains(BLOCK_START) {
                let new = remove_block(&content, BLOCK_START, BLOCK_END);
                fs::write(&rc_path, new)
                    .with_context(|| format!("failed to write {}", rc_path.display()))?;
                println!("🗑  Removed shell hooks from {}", rc_path.display());
                any = true;
            }
        }
    }

    // CLAUDE.md
    let claude_path = PathBuf::from("CLAUDE.md");
    if let Ok(content) = fs::read_to_string(&claude_path) {
        if content.contains(CLAUDE_MD_BLOCK_START) {
            let new = remove_block(&content, CLAUDE_MD_BLOCK_START, CLAUDE_MD_BLOCK_END);
            fs::write(&claude_path, new)
                .with_context(|| "failed to write CLAUDE.md")?;
            println!("🗑  Removed sift block from CLAUDE.md");
            any = true;
        }
    }

    // copilot-instructions.md
    let copilot_path = PathBuf::from(".github/copilot-instructions.md");
    if let Ok(content) = fs::read_to_string(&copilot_path) {
        if content.contains(COPILOT_BLOCK_START) {
            let new = remove_block(&content, COPILOT_BLOCK_START, COPILOT_BLOCK_END);
            fs::write(&copilot_path, new)
                .with_context(|| "failed to write .github/copilot-instructions.md")?;
            println!("🗑  Removed sift block from .github/copilot-instructions.md");
            any = true;
        }
    }

    if !any {
        println!("Nothing to uninstall — no sift hooks found.");
    }

    Ok(())
}

// ── Status ────────────────────────────────────────────────────────────────────

fn show_status() -> Result<()> {
    println!("Sift init status");
    println!("{}", "─".repeat(40));

    // Shell hook
    let shell_status = if let Ok(rc_path) = detect_rc_file() {
        if let Ok(content) = fs::read_to_string(&rc_path) {
            if content.contains(BLOCK_START) {
                format!("✅ installed  ({})", rc_path.display())
            } else {
                "✗  not installed  (run: sift init --shell)".to_string()
            }
        } else {
            "✗  rc file not readable".to_string()
        }
    } else {
        "✗  could not detect shell rc file".to_string()
    };
    println!("  Shell hooks:  {shell_status}");

    // CLAUDE.md
    let claude_status = if let Ok(content) = fs::read_to_string("CLAUDE.md") {
        if content.contains(CLAUDE_MD_BLOCK_START) {
            "✅ installed  (CLAUDE.md)".to_string()
        } else {
            "✗  not installed  (run: sift init --claude)".to_string()
        }
    } else {
        "✗  CLAUDE.md not found  (run: sift init --claude)".to_string()
    };
    println!("  CLAUDE.md:    {claude_status}");

    // copilot-instructions
    let copilot_status =
        if let Ok(content) = fs::read_to_string(".github/copilot-instructions.md") {
            if content.contains(COPILOT_BLOCK_START) {
                "✅ installed  (.github/copilot-instructions.md)".to_string()
            } else {
                "✗  not installed  (run: sift init --copilot)".to_string()
            }
        } else {
            "✗  not found  (run: sift init --copilot)".to_string()
        };
    println!("  Copilot:      {copilot_status}");

    println!();
    println!("  Install all:  sift init --shell --claude --copilot");
    println!("  Uninstall:    sift init --uninstall");

    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn home_dir() -> Result<PathBuf> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .with_context(|| "$HOME not set")
}

/// Replace the content between start/end markers (inclusive) with `block`.
fn replace_block(source: &str, start: &str, end: &str, block: &str) -> String {
    let start_idx = match source.find(start) {
        Some(i) => i,
        None => return format!("{source}\n{block}"),
    };
    let end_idx = match source.find(end) {
        Some(i) => i + end.len(),
        None => return format!("{source}\n{block}"),
    };
    // Consume trailing newline after end marker if present
    let after_end = if source.as_bytes().get(end_idx) == Some(&b'\n') {
        end_idx + 1
    } else {
        end_idx
    };
    format!("{}{}{}", &source[..start_idx], block, &source[after_end..])
}

/// Remove the block between start/end markers (inclusive) and any blank line before it.
fn remove_block(source: &str, start: &str, end: &str) -> String {
    let start_idx = match source.find(start) {
        Some(i) => i,
        None => return source.to_string(),
    };
    let end_idx = match source.find(end) {
        Some(i) => i + end.len(),
        None => return source.to_string(),
    };
    let after_end = if source.as_bytes().get(end_idx) == Some(&b'\n') {
        end_idx + 1
    } else {
        end_idx
    };
    // Trim trailing blank line before the block
    let before = source[..start_idx].trim_end_matches('\n');
    let after = &source[after_end..];
    if after.is_empty() {
        format!("{before}\n")
    } else {
        format!("{before}\n{after}")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_block_substitutes_existing_block() {
        let source = "before\n# BEGIN sift hooks\nold content\n# END sift hooks\nafter\n";
        let new_block = "# BEGIN sift hooks\nnew content\n# END sift hooks\n";
        let result = replace_block(source, BLOCK_START, BLOCK_END, new_block);
        assert!(result.contains("new content"));
        assert!(!result.contains("old content"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    #[test]
    fn replace_block_appends_when_no_existing_block() {
        let source = "existing content\n";
        let block = "# BEGIN sift hooks\nnew\n# END sift hooks\n";
        let result = replace_block(source, BLOCK_START, BLOCK_END, block);
        assert!(result.contains("existing content"));
        assert!(result.contains("new"));
    }

    #[test]
    fn remove_block_strips_markers_and_content() {
        let source = "before\n# BEGIN sift hooks\ncontent\n# END sift hooks\nafter\n";
        let result = remove_block(source, BLOCK_START, BLOCK_END);
        assert!(!result.contains(BLOCK_START));
        assert!(!result.contains("content"));
        assert!(result.contains("before"));
        assert!(result.contains("after"));
    }

    #[test]
    fn remove_block_is_noop_when_no_block() {
        let source = "no hooks here\n";
        let result = remove_block(source, BLOCK_START, BLOCK_END);
        assert_eq!(result, source);
    }

    #[test]
    fn build_shell_block_contains_all_commands() {
        let block = build_shell_block();
        assert!(block.contains(BLOCK_START));
        assert!(block.contains(BLOCK_END));
        assert!(block.contains("git()"));
        assert!(block.contains("xcodebuild()"));
        assert!(block.contains("xcrun()"));
        assert!(block.contains("swiftlint()"));
    }

    #[test]
    fn replace_then_remove_is_idempotent() {
        let original = "top\n";
        let block = build_shell_block();
        let with_block = replace_block(original, BLOCK_START, BLOCK_END, &block);
        let without = remove_block(&with_block, BLOCK_START, BLOCK_END);
        assert!(!without.contains(BLOCK_START));
        assert!(without.contains("top"));
    }
}
