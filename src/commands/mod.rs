#![allow(dead_code)] // Stub: wired into main.rs in Phase 4

pub mod git;
pub mod grep;
pub mod read;
pub mod xcodebuild;

use git::GitSubcommand;
use xcodebuild::XcodebuildSubcommand;

/// The family of the command being proxied.
///
/// Used to select the appropriate output filter.
/// `Unknown` triggers safe passthrough — no filtering applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandFamily {
    Git(GitSubcommand),
    Grep,
    Read,
    Xcodebuild(XcodebuildSubcommand),
    /// Command not recognized — passed through unmodified.
    Unknown,
}

/// Detect the command family from the argument list.
///
/// Returns `CommandFamily::Unknown` for unrecognized commands.
/// Safe passthrough is always the fallback — Sift never blocks a command.
pub fn detect(args: &[String]) -> CommandFamily {
    let program = match args.first() {
        Some(p) => p.as_str(),
        None => return CommandFamily::Unknown,
    };

    match program {
        "git" => CommandFamily::Git(git::detect_subcommand(args)),
        "grep" | "rg" | "ripgrep" => CommandFamily::Grep,
        "cat" | "less" | "head" | "tail" => CommandFamily::Read,
        "xcodebuild" => CommandFamily::Xcodebuild(xcodebuild::detect_subcommand(args)),
        _ => CommandFamily::Unknown,
    }
}
