/// Subcommands for `swift build` and `swift test` (SPM).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwiftBuildSubcommand {
    Build,
    Test,
    /// Any other `swift` invocation not handled elsewhere.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> SwiftBuildSubcommand {
    match args.get(1).map(|s| s.as_str()) {
        Some("build") => SwiftBuildSubcommand::Build,
        Some("test") => SwiftBuildSubcommand::Test,
        _ => SwiftBuildSubcommand::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_build() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "build"])),
            SwiftBuildSubcommand::Build
        );
    }

    #[test]
    fn detects_build_with_flags() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "build", "--release"])),
            SwiftBuildSubcommand::Build
        );
    }

    #[test]
    fn detects_test() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "test"])),
            SwiftBuildSubcommand::Test
        );
    }

    #[test]
    fn detects_test_with_flags() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "test", "--filter", "MyTests"])),
            SwiftBuildSubcommand::Test
        );
    }

    #[test]
    fn other_subcommand_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "run"])),
            SwiftBuildSubcommand::Other
        );
    }

    #[test]
    fn bare_swift_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["swift"])),
            SwiftBuildSubcommand::Other
        );
    }
}
