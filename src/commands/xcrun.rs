/// Xcrun subcommands that Sift has specialized filters for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XcrunSubcommand {
    SimctlList,
    SimctlBoot,
    SimctlInstall,
    SimctlLaunch,
    SimctlErase,
    SimctlDelete,
    /// Any other xcrun invocation — passed through unfiltered.
    Other,
}

/// Detect the xcrun subcommand from the argument list.
pub fn detect_subcommand(args: &[String]) -> XcrunSubcommand {
    // args[0] = "xcrun", args[1] = "simctl", args[2] = sub-subcommand
    match (
        args.get(1).map(String::as_str),
        args.get(2).map(String::as_str),
    ) {
        (Some("simctl"), Some("list")) => XcrunSubcommand::SimctlList,
        (Some("simctl"), Some("boot")) => XcrunSubcommand::SimctlBoot,
        (Some("simctl"), Some("install")) => XcrunSubcommand::SimctlInstall,
        (Some("simctl"), Some("launch")) => XcrunSubcommand::SimctlLaunch,
        (Some("simctl"), Some("erase")) => XcrunSubcommand::SimctlErase,
        (Some("simctl"), Some("delete")) => XcrunSubcommand::SimctlDelete,
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
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "list", "--json"])),
            XcrunSubcommand::SimctlList
        );
    }

    #[test]
    fn detects_simctl_boot() {
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "boot", "UDID-123"])),
            XcrunSubcommand::SimctlBoot
        );
    }

    #[test]
    fn detects_simctl_install() {
        assert_eq!(
            detect_subcommand(&args(&[
                "xcrun",
                "simctl",
                "install",
                "UDID-123",
                "MyApp.app"
            ])),
            XcrunSubcommand::SimctlInstall
        );
    }

    #[test]
    fn detects_simctl_launch() {
        assert_eq!(
            detect_subcommand(&args(&[
                "xcrun",
                "simctl",
                "launch",
                "UDID-123",
                "com.example"
            ])),
            XcrunSubcommand::SimctlLaunch
        );
    }

    #[test]
    fn detects_simctl_erase() {
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "erase", "UDID-123"])),
            XcrunSubcommand::SimctlErase
        );
    }

    #[test]
    fn detects_simctl_delete() {
        assert_eq!(
            detect_subcommand(&args(&["xcrun", "simctl", "delete", "UDID-123"])),
            XcrunSubcommand::SimctlDelete
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
