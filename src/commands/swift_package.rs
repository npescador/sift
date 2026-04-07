/// Swift Package Manager subcommands that Sift has specialized filters for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SwiftPackageSubcommand {
    Resolve,
    Update,
    /// `swift package show-dependencies`
    ShowDependencies,
    /// Any other `swift package` invocation — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> SwiftPackageSubcommand {
    // args[0] = "swift", args[1] = "package", args[2] = subcommand
    match args.get(2).map(|s| s.as_str()) {
        Some("resolve") => SwiftPackageSubcommand::Resolve,
        Some("update") => SwiftPackageSubcommand::Update,
        Some("show-dependencies") => SwiftPackageSubcommand::ShowDependencies,
        _ => SwiftPackageSubcommand::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_resolve() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "package", "resolve"])),
            SwiftPackageSubcommand::Resolve
        );
    }

    #[test]
    fn detects_update() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "package", "update"])),
            SwiftPackageSubcommand::Update
        );
    }

    #[test]
    fn detects_show_dependencies() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "package", "show-dependencies"])),
            SwiftPackageSubcommand::ShowDependencies
        );
    }

    #[test]
    fn other_subcommand_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["swift", "package", "clean"])),
            SwiftPackageSubcommand::Other
        );
    }
}
