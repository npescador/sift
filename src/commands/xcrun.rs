/// Xcrun subcommands that Sift has specialized filters for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XcrunSubcommand {
    SimctlList,
    /// Any other xcrun invocation — passed through unfiltered.
    Other,
}

/// Detect the xcrun subcommand from the argument list.
///
/// `xcrun simctl list` is the only subcommand with a dedicated filter.
/// Everything else passes through unmodified.
pub fn detect_subcommand(args: &[String]) -> XcrunSubcommand {
    // args[0] = "xcrun", args[1] = subcommand, args[2] = sub-subcommand
    match (
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) {
        (Some("simctl"), Some("list")) => XcrunSubcommand::SimctlList,
        _ => XcrunSubcommand::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_simctl_list() {
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "list"])),
            XcrunSubcommand::SimctlList
        );
    }

    #[test]
    fn simctl_list_with_flags_detected() {
        // Extra flags after "list" are fine — filter handles them
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "list", "--json"])),
            XcrunSubcommand::SimctlList
        );
    }

    #[test]
    fn other_xcrun_subcommand_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "swift", "-version"])),
            XcrunSubcommand::Other
        );
    }

    #[test]
    fn bare_xcrun_returns_other() {
        assert_eq!(detect_subcommand(&args(&["xcrun"])), XcrunSubcommand::Other);
    }
}
