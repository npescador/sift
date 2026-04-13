//! # sift-lib
//!
//! Programmatic library API for [Sift](https://github.com/npescador/sift) — a smart output
//! reduction layer for AI coding workflows.
//!
//! Sift filters verbose shell command output down to high-signal summaries, reducing the token
//! cost of feeding terminal output to AI coding agents.
//!
//! ## Quick start
//!
//! ```rust,no_run
//! use sift_lib::{filter, run, Verbosity};
//!
//! // Filter pre-captured output
//! let raw = "M  src/main.rs\n?? scratch.txt\n";
//! let out = filter(&["git", "status"], raw, Verbosity::Compact);
//! println!("{}", out.content);
//!
//! // Execute a command and get filtered output in one call
//! let result = run(&["git", "status"], Verbosity::Compact).unwrap();
//! println!("exit={} filtered={}", result.exit_code, result.filtered.content);
//! ```

// Re-export core types so callers only need `sift_lib::*`.
pub use sift_cli::commands::CommandFamily;
pub use sift_cli::error::SiftError;
pub use sift_cli::filters::{FilterOutput, Verbosity};

use sift_cli::commands;
use sift_cli::executor;

/// The result of executing a command through Sift.
pub struct RunResult {
    /// Filtered stdout.
    pub filtered: FilterOutput,
    /// Raw stderr from the command (passed through unmodified).
    pub stderr: String,
    /// Exit code from the subprocess — always the exact code, never modified.
    pub exit_code: i32,
    /// Wall-clock duration of the subprocess in milliseconds.
    pub duration_ms: u64,
}

/// Filter pre-captured stdout for the given command arguments.
///
/// Use this when you already have the raw output and just want the filtered summary.
/// No subprocess is spawned.
///
/// # Arguments
/// * `args`      — full command and arguments, e.g. `&["git", "diff"]`
/// * `stdout`    — raw stdout captured from the command
/// * `verbosity` — how aggressively to reduce the output
///
/// # Example
/// ```rust
/// use sift_lib::{filter, Verbosity};
///
/// let raw = "error: use of unresolved identifier 'Foo'\n";
/// let out = filter(&["xcodebuild", "build"], raw, Verbosity::Compact);
/// assert!(!out.content.is_empty() || out.original_bytes > 0);
/// ```
pub fn filter(args: &[&str], stdout: &str, verbosity: Verbosity) -> FilterOutput {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let family = commands::detect(&owned);
    apply_filter(&owned, stdout, verbosity, &family)
}

/// Execute `args[0]` with the remaining arguments and return filtered output.
///
/// Spawns a subprocess, captures stdout/stderr, applies the Sift filter, and returns
/// the [`RunResult`]. The exit code is always propagated exactly as-is.
///
/// # Errors
/// Returns [`SiftError::CommandNotFound`] if the binary is not on `$PATH`.
/// Returns [`SiftError::Io`] for other spawn failures.
///
/// # Example
/// ```rust,no_run
/// use sift_lib::{run, Verbosity};
///
/// let result = run(&["git", "status"], Verbosity::Compact).unwrap();
/// println!("{}", result.filtered.content);
/// assert_eq!(result.exit_code, 0);
/// ```
pub fn run(args: &[&str], verbosity: Verbosity) -> Result<RunResult, SiftError> {
    if args.is_empty() {
        return Err(SiftError::CommandNotFound(String::new()));
    }
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let family = commands::detect(&owned);
    let output = executor::execute(&owned[0], &owned[1..])?;
    let filtered = apply_filter(&owned, &output.stdout, verbosity, &family);
    Ok(RunResult {
        filtered,
        stderr: output.stderr,
        exit_code: output.exit_code,
        duration_ms: output.duration_ms,
    })
}

/// Detect which [`CommandFamily`] a set of command arguments belongs to.
///
/// Useful when you want to know how Sift will classify a command before filtering.
///
/// # Example
/// ```rust
/// use sift_lib::{detect_family, CommandFamily};
///
/// let family = detect_family(&["xcodebuild", "test", "-scheme", "MyApp"]);
/// assert!(matches!(family, CommandFamily::Xcodebuild(_)));
/// ```
pub fn detect_family(args: &[&str]) -> CommandFamily {
    let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    commands::detect(&owned)
}

// Internal routing — mirrors main.rs apply_filter but lives in the library.
fn apply_filter(
    args: &[String],
    stdout: &str,
    verbosity: Verbosity,
    family: &CommandFamily,
) -> FilterOutput {
    use sift_cli::filters::*;
    use CommandFamily::*;

    if verbosity == Verbosity::Raw {
        return FilterOutput::passthrough(stdout);
    }

    match family {
        Git(sub) => match sub {
            commands::git::GitSubcommand::Status => git_status::filter(stdout, verbosity),
            commands::git::GitSubcommand::Diff => git_diff::filter(stdout, verbosity),
            commands::git::GitSubcommand::Log => git_log::filter(stdout, verbosity),
            commands::git::GitSubcommand::LogGraph => git_log::filter_graph(stdout, verbosity),
            commands::git::GitSubcommand::Other => FilterOutput::passthrough(stdout),
        },
        Grep => grep::filter(stdout, verbosity),
        Read => read::filter(stdout, verbosity),
        Ls => ls_xcode::filter_ls(stdout, verbosity),
        Find => ls_xcode::filter_find(stdout, verbosity),
        Curl => curl::filter(stdout, verbosity),
        Xcodebuild(sub) => match sub {
            commands::xcodebuild::XcodebuildSubcommand::Build => {
                xcodebuild_build::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Test => {
                xcodebuild_test::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::ShowBuildSettings => {
                xcodebuild_settings::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Archive => {
                xcodebuild_archive::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::List => {
                xcodebuild_list::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Other => FilterOutput::passthrough(stdout),
        },
        Xcrun(sub) => match sub {
            commands::xcrun::XcrunSubcommand::SimctlList => xcrun_simctl::filter(stdout, verbosity),
            commands::xcrun::XcrunSubcommand::SimctlBoot
            | commands::xcrun::XcrunSubcommand::SimctlInstall
            | commands::xcrun::XcrunSubcommand::SimctlLaunch
            | commands::xcrun::XcrunSubcommand::SimctlErase
            | commands::xcrun::XcrunSubcommand::SimctlDelete => {
                xcrun_simctl::filter_simctl_action(stdout, verbosity)
            }
            commands::xcrun::XcrunSubcommand::Other => FilterOutput::passthrough(stdout),
        },
        XcResultTool => xcresulttool::filter(stdout, verbosity),
        DocC => docc::filter(stdout, verbosity),
        Swiftlint => swiftlint::filter(stdout, verbosity),
        Fastlane => fastlane::filter(stdout, verbosity),
        SwiftFormat => swiftformat::filter(stdout, verbosity),
        SwiftPackage(_) => swift_package::filter(stdout, verbosity),
        Pod(_) => pod::filter(stdout, verbosity),
        Tuist(_) => tuist::filter(stdout, verbosity),
        Codesign => codesign::filter(stdout, verbosity),
        Security => codesign::filter_security(stdout, verbosity),
        Agvtool => agvtool::filter(stdout, verbosity),
        XcodeSelect => xcode_select::filter(stdout, verbosity),
        SwiftBuild(sub) => match sub {
            commands::swift_build::SwiftBuildSubcommand::Build => {
                swift_build::filter(stdout, verbosity)
            }
            commands::swift_build::SwiftBuildSubcommand::Test => {
                swift_test::filter(stdout, verbosity)
            }
            commands::swift_build::SwiftBuildSubcommand::Other => FilterOutput::passthrough(stdout),
        },
        Unknown => {
            let _ = args;
            FilterOutput::passthrough(stdout)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_git_status_compact_reduces_output() {
        let raw = " M src/main.rs\n?? scratch.txt\n";
        let out = filter(&["git", "status"], raw, Verbosity::Compact);
        assert!(out.original_bytes > 0);
        assert_eq!(out.original_bytes, raw.len());
    }

    #[test]
    fn filter_raw_verbosity_passes_through() {
        let raw = "some verbose output\nthat should not be filtered\n";
        let out = filter(&["git", "status"], raw, Verbosity::Raw);
        assert_eq!(out.content, raw);
        assert_eq!(out.original_bytes, out.filtered_bytes);
    }

    #[test]
    fn filter_unknown_command_passes_through() {
        let raw = "output from an unknown command\n";
        let out = filter(&["myunknowncmd", "--flag"], raw, Verbosity::Compact);
        assert_eq!(out.content, raw);
    }

    #[test]
    fn detect_family_identifies_xcodebuild() {
        let family = detect_family(&["xcodebuild", "test", "-scheme", "MyApp"]);
        assert!(matches!(family, CommandFamily::Xcodebuild(_)));
    }

    #[test]
    fn detect_family_identifies_git() {
        let family = detect_family(&["git", "diff"]);
        assert!(matches!(family, CommandFamily::Git(_)));
    }

    #[test]
    fn detect_family_unknown_for_unrecognised_command() {
        let family = detect_family(&["somerandombinary"]);
        assert!(matches!(family, CommandFamily::Unknown));
    }

    #[test]
    fn filter_returns_filter_output_with_correct_byte_counts() {
        let raw = "BUILD FAILED\n\nCompileSwift normal arm64\n  error: something went wrong\n";
        let out = filter(&["xcodebuild", "build"], raw, Verbosity::Compact);
        assert_eq!(out.original_bytes, raw.len());
    }
}
