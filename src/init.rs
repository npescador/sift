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
    pub xcode_project: bool,
    pub show: bool,
    pub uninstall: bool,
}

/// Entry point called from `main.rs`.
pub fn run(opts: InitOptions) -> Result<()> {
    // Default: if no flag specified, treat as --show
    if !opts.shell && !opts.claude && !opts.copilot && !opts.xcode_project && !opts.uninstall {
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
    if opts.xcode_project {
        install_xcode_project_context()?;
    }

    Ok(())
}

// ── Shell hook ───────────────────────────────────────────────────────────────

fn install_shell_hook() -> Result<()> {
    let rc_path = detect_rc_file()?;
    let current = fs::read_to_string(&rc_path).unwrap_or_default();

    let block = build_shell_block();

    let new_content = if current.contains(BLOCK_START) {
        replace_block(&current, BLOCK_START, BLOCK_END, &block)
    } else {
        format!("{}\n{}\n", current.trim_end_matches('\n'), block)
    };

    fs::write(&rc_path, new_content)
        .with_context(|| format!("failed to write {}", rc_path.display()))?;

    println!("✅ Shell hooks installed in {}", rc_path.display());
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

    let block = format!("{CLAUDE_MD_BLOCK_START}\n{CLAUDE_MD_CONTENT}\n{CLAUDE_MD_BLOCK_END}\n");

    let new_content = if current.contains(CLAUDE_MD_BLOCK_START) {
        replace_block(&current, CLAUDE_MD_BLOCK_START, CLAUDE_MD_BLOCK_END, &block)
    } else if current.is_empty() {
        block
    } else {
        format!("{}\n\n{}", current.trim_end_matches('\n'), block)
    };

    fs::write(&path, new_content).with_context(|| "failed to write CLAUDE.md")?;

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

    let block = format!("{COPILOT_BLOCK_START}\n{COPILOT_CONTENT}\n{COPILOT_BLOCK_END}\n");

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

// ── Xcode project context ────────────────────────────────────────────────────

/// Marker for the xcode-project block in CLAUDE.md.
const XCODE_BLOCK_START: &str = "<!-- BEGIN sift xcode-project -->";
const XCODE_BLOCK_END: &str = "<!-- END sift xcode-project -->";

fn install_xcode_project_context() -> Result<()> {
    let info = detect_xcode_project()?;
    let block = build_xcode_block(&info);

    let path = PathBuf::from("CLAUDE.md");
    let current = fs::read_to_string(&path).unwrap_or_default();

    let new_content = if current.contains(XCODE_BLOCK_START) {
        replace_block(&current, XCODE_BLOCK_START, XCODE_BLOCK_END, &block)
    } else {
        format!("{}\n\n{}", current.trim_end_matches('\n'), block)
    };

    fs::write(&path, new_content).with_context(|| "failed to write CLAUDE.md")?;

    println!("✅ CLAUDE.md updated with Xcode project context");
    println!();
    println!("   Project:  {}", info.name);
    if let Some(ref scheme) = info.default_scheme {
        println!("   Scheme:   {scheme}");
    }
    if !info.targets.is_empty() {
        println!("   Targets:  {}", info.targets.join(", "));
    }
    if let Some(ref dest) = info.simulator_destination {
        println!("   Dest:     {dest}");
    }
    Ok(())
}

/// Information extracted from the Xcode project.
#[derive(Debug)]
struct XcodeProjectInfo {
    name: String,
    default_scheme: Option<String>,
    targets: Vec<String>,
    simulator_destination: Option<String>,
}

/// Detect and parse the nearest .xcodeproj / .xcworkspace.
fn detect_xcode_project() -> Result<XcodeProjectInfo> {
    let cwd = std::env::current_dir().with_context(|| "cannot read current directory")?;

    // Prefer .xcworkspace (CocoaPods / multi-project setups), then .xcodeproj
    let workspace = find_extension(&cwd, "xcworkspace");
    let xcodeproj = find_extension(&cwd, "xcodeproj");

    let project_path = workspace.or(xcodeproj);

    let name = project_path
        .as_ref()
        .and_then(|p| p.file_stem())
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            cwd.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("MyApp")
                .to_string()
        });

    // Try to get schemes and targets from xcodebuild -list
    let (schemes, targets) = run_xcodebuild_list(&name);

    let default_scheme = schemes.first().cloned().or_else(|| Some(name.clone()));

    // Pick a sensible simulator destination
    let simulator_destination = pick_simulator_destination();

    Ok(XcodeProjectInfo {
        name,
        default_scheme,
        targets,
        simulator_destination,
    })
}

/// Find the first file with the given extension in `dir`.
fn find_extension(dir: &Path, ext: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.find_map(|entry| {
        let path = entry.ok()?.path();
        if path.extension()?.to_str()? == ext {
            Some(path)
        } else {
            None
        }
    })
}

/// Run `xcodebuild -list` and parse schemes and targets.
/// Returns empty vecs if xcodebuild is unavailable or fails.
fn run_xcodebuild_list(project_name: &str) -> (Vec<String>, Vec<String>) {
    let output = std::process::Command::new("xcodebuild")
        .args(["-list"])
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return (vec![project_name.to_string()], vec![]),
    };

    let schemes = parse_xcodebuild_list_section(&stdout, "Schemes:");
    let targets = parse_xcodebuild_list_section(&stdout, "Targets:");
    (schemes, targets)
}

/// Parse a section from `xcodebuild -list` output.
/// Each item is indented with spaces under the section header.
fn parse_xcodebuild_list_section(output: &str, section: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in output.lines() {
        if line.trim() == section.trim_end_matches(':') || line.trim_end() == section {
            in_section = true;
            continue;
        }
        if in_section {
            let trimmed = line.trim();
            if trimmed.is_empty() || (!line.starts_with("    ") && !line.starts_with('\t')) {
                break;
            }
            if !trimmed.is_empty() {
                items.push(trimmed.to_string());
            }
        }
    }
    items
}

/// Pick the most relevant simulator destination string.
fn pick_simulator_destination() -> Option<String> {
    // Use xcrun simctl list to find a booted iPhone simulator
    let output = std::process::Command::new("xcrun")
        .args(["simctl", "list", "devices", "booted"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    for line in stdout.lines() {
        if line.contains("iPhone") && line.contains("Booted") {
            // Extract device name: "    iPhone 16 Pro (UUID) (Booted)"
            let name = line.trim().split('(').next()?.trim().to_string();
            if !name.is_empty() {
                return Some(format!("platform=iOS Simulator,name={name}"));
            }
        }
    }

    // Fallback: generic latest iPhone
    Some("platform=iOS Simulator,name=iPhone 16 Pro".to_string())
}

/// Build the CLAUDE.md xcode-project block content.
fn build_xcode_block(info: &XcodeProjectInfo) -> String {
    let scheme = info.default_scheme.as_deref().unwrap_or(&info.name);
    let dest = info
        .simulator_destination
        .as_deref()
        .unwrap_or("platform=iOS Simulator,name=iPhone 16 Pro");

    let targets_line = if info.targets.is_empty() {
        String::new()
    } else {
        format!("\nTargets: {}", info.targets.join(", "))
    };

    format!(
        "{XCODE_BLOCK_START}\n\
         ## Xcode Project: {name}\n\
         \n\
         Scheme: `{scheme}`{targets_line}\n\
         \n\
         ### Common sift commands for this project\n\
         \n\
         ```bash\n\
         sift xcodebuild build -scheme {scheme} -destination \"{dest}\"\n\
         sift xcodebuild test  -scheme {scheme} -destination \"{dest}\"\n\
         sift xcodebuild archive -scheme {scheme}\n\
         sift xcodebuild -showBuildSettings -scheme {scheme}\n\
         sift xcrun simctl list\n\
         sift git status\n\
         sift git diff\n\
         sift git log\n\
         sift swiftlint lint\n\
         ```\n\
         \n\
         Always use `sift` prefix for compact output. Add `-v` for more detail.\n\
         {XCODE_BLOCK_END}\n",
        name = info.name,
    )
}

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
            fs::write(&claude_path, new).with_context(|| "failed to write CLAUDE.md")?;
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
    let copilot_status = if let Ok(content) = fs::read_to_string(".github/copilot-instructions.md")
    {
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

    #[test]
    fn build_xcode_block_contains_scheme_and_commands() {
        let info = XcodeProjectInfo {
            name: "MyApp".to_string(),
            default_scheme: Some("MyApp".to_string()),
            targets: vec!["MyApp".to_string(), "MyAppTests".to_string()],
            simulator_destination: Some("platform=iOS Simulator,name=iPhone 16 Pro".to_string()),
        };
        let block = build_xcode_block(&info);
        assert!(block.contains(XCODE_BLOCK_START));
        assert!(block.contains(XCODE_BLOCK_END));
        assert!(block.contains("MyApp"));
        assert!(block.contains("sift xcodebuild build"));
        assert!(block.contains("sift xcodebuild test"));
        assert!(block.contains("iPhone 16 Pro"));
    }

    #[test]
    fn parse_xcodebuild_list_section_extracts_items() {
        let output = "Information about project \"MyApp\":\n\
            Targets:\n\
            \tMyApp\n\
            \tMyAppTests\n\
            \n\
            Schemes:\n\
            \tMyApp\n\
            \tMyApp-Dev\n";
        let targets = parse_xcodebuild_list_section(output, "Targets:");
        let schemes = parse_xcodebuild_list_section(output, "Schemes:");
        assert_eq!(targets, vec!["MyApp", "MyAppTests"]);
        assert_eq!(schemes, vec!["MyApp", "MyApp-Dev"]);
    }

    #[test]
    fn xcode_block_is_replaceable() {
        let info = XcodeProjectInfo {
            name: "TestApp".to_string(),
            default_scheme: Some("TestApp".to_string()),
            targets: vec![],
            simulator_destination: None,
        };
        let block = build_xcode_block(&info);
        let existing = format!("# Existing\n\n{block}");
        let new_info = XcodeProjectInfo {
            name: "UpdatedApp".to_string(),
            default_scheme: Some("UpdatedApp".to_string()),
            targets: vec![],
            simulator_destination: None,
        };
        let new_block = build_xcode_block(&new_info);
        let updated = replace_block(&existing, XCODE_BLOCK_START, XCODE_BLOCK_END, &new_block);
        assert!(updated.contains("UpdatedApp"));
        assert!(!updated.contains("TestApp"));
        assert!(updated.contains("# Existing"));
    }
}
