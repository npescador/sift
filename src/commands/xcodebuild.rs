/// Xcodebuild subcommands that Sift has specialized filters for.
///
/// xcodebuild uses positional action keywords (`build`, `test`) optionally
/// mixed with flags. Detection scans all args, not just position 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XcodebuildSubcommand {
    Build,
    Test,
    /// Any other xcodebuild invocation — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> XcodebuildSubcommand {
    for arg in args.iter().skip(1) {
        match arg.as_str() {
            "build" | "build-for-testing" => return XcodebuildSubcommand::Build,
            "test" | "test-without-building" => return XcodebuildSubcommand::Test,
            _ => {}
        }
    }
    XcodebuildSubcommand::Other
}
