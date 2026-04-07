pub mod curl;
pub mod git;
pub mod grep;
pub mod read;
pub mod swift_build;
pub mod swift_package;
pub mod xcodebuild;
pub mod xcrun;

use git::GitSubcommand;
use swift_build::SwiftBuildSubcommand;
use swift_package::SwiftPackageSubcommand;
use xcodebuild::XcodebuildSubcommand;
use xcrun::XcrunSubcommand;

/// The family of the command being proxied.
///
/// Used to select the appropriate output filter.
/// `Unknown` triggers safe passthrough — no filtering applied.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    Fastlane,
    SwiftPackage(SwiftPackageSubcommand),
    SwiftBuild(SwiftBuildSubcommand),
    /// Command not recognized — passed through unmodified.
    Unknown,
}

impl CommandFamily {
    /// Return a short lowercase string identifying the command family.
    pub fn name(&self) -> &'static str {
        match self {
            CommandFamily::Git(_) => "git",
            CommandFamily::Grep => "grep",
            CommandFamily::Read => "read",
            CommandFamily::Ls => "ls",
            CommandFamily::Find => "find",
            CommandFamily::Curl => "curl",
            CommandFamily::Xcodebuild(_) => "xcodebuild",
            CommandFamily::Xcrun(_) => "xcrun",
            CommandFamily::Swiftlint => "swiftlint",
            CommandFamily::Fastlane => "fastlane",
            CommandFamily::SwiftPackage(_) => "swift",
            CommandFamily::SwiftBuild(_) => "swift",
            CommandFamily::Unknown => "unknown",
        }
    }
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
        "ls" | "eza" | "exa" => CommandFamily::Ls,
        "find" => CommandFamily::Find,
        "curl" => CommandFamily::Curl,
        "xcodebuild" => CommandFamily::Xcodebuild(xcodebuild::detect_subcommand(args)),
        "xcrun" => CommandFamily::Xcrun(xcrun::detect_subcommand(args)),
        "swiftlint" => CommandFamily::Swiftlint,
        "fastlane" => CommandFamily::Fastlane,
        "swift" if args.get(1).map(|s| s.as_str()) == Some("package") => {
            CommandFamily::SwiftPackage(swift_package::detect_subcommand(args))
        }
        "swift"
            if args.get(1).map(|s| s.as_str()) == Some("build")
                || args.get(1).map(|s| s.as_str()) == Some("test") =>
        {
            CommandFamily::SwiftBuild(swift_build::detect_subcommand(args))
        }
        _ => CommandFamily::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detect_git_returns_git_family() {
        assert!(matches!(
            detect(&args(&["git", "status"])),
            CommandFamily::Git(_)
        ));
    }

    #[test]
    fn detect_rg_returns_grep_family() {
        assert_eq!(detect(&args(&["rg", "pattern"])), CommandFamily::Grep);
    }

    #[test]
    fn detect_grep_returns_grep_family() {
        assert_eq!(detect(&args(&["grep", "pattern"])), CommandFamily::Grep);
    }

    #[test]
    fn detect_cat_returns_read_family() {
        assert_eq!(detect(&args(&["cat", "file.txt"])), CommandFamily::Read);
    }

    #[test]
    fn detect_xcodebuild_returns_xcodebuild_family() {
        assert!(matches!(
            detect(&args(&["xcodebuild", "build"])),
            CommandFamily::Xcodebuild(_)
        ));
    }

    #[test]
    fn detect_unknown_program_returns_unknown() {
        assert_eq!(detect(&args(&["cargo"])), CommandFamily::Unknown);
    }

    #[test]
    fn detect_empty_args_returns_unknown() {
        let empty: Vec<String> = vec![];
        assert_eq!(detect(&empty), CommandFamily::Unknown);
    }

    #[test]
    fn name_returns_correct_string_for_each_variant() {
        assert_eq!(CommandFamily::Git(git::GitSubcommand::Status).name(), "git");
        assert_eq!(CommandFamily::Grep.name(), "grep");
        assert_eq!(CommandFamily::Read.name(), "read");
        assert_eq!(
            CommandFamily::Xcodebuild(xcodebuild::XcodebuildSubcommand::Build).name(),
            "xcodebuild"
        );
        assert_eq!(CommandFamily::Fastlane.name(), "fastlane");
        assert_eq!(
            CommandFamily::SwiftPackage(swift_package::SwiftPackageSubcommand::Resolve).name(),
            "swift"
        );
        assert_eq!(CommandFamily::Unknown.name(), "unknown");
    }

    #[test]
    fn detect_fastlane_returns_fastlane_family() {
        assert_eq!(
            detect(&args(&["fastlane", "beta"])),
            CommandFamily::Fastlane
        );
    }

    #[test]
    fn detect_swift_package_resolve() {
        assert!(matches!(
            detect(&args(&["swift", "package", "resolve"])),
            CommandFamily::SwiftPackage(_)
        ));
    }

    #[test]
    fn detect_swift_build_is_swift_build_family() {
        assert!(matches!(
            detect(&args(&["swift", "build"])),
            CommandFamily::SwiftBuild(_)
        ));
    }
}
