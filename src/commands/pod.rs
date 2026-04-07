/// CocoaPods `pod` subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodSubcommand {
    Install,
    Update,
    /// Any other `pod` invocation — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> PodSubcommand {
    match args.get(1).map(|s| s.as_str()) {
        Some("install") => PodSubcommand::Install,
        Some("update") => PodSubcommand::Update,
        _ => PodSubcommand::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_install() {
        assert_eq!(
            detect_subcommand(&args(&["pod", "install"])),
            PodSubcommand::Install
        );
    }

    #[test]
    fn detects_update() {
        assert_eq!(
            detect_subcommand(&args(&["pod", "update"])),
            PodSubcommand::Update
        );
    }

    #[test]
    fn detects_update_with_pod_name() {
        assert_eq!(
            detect_subcommand(&args(&["pod", "update", "Alamofire"])),
            PodSubcommand::Update
        );
    }

    #[test]
    fn other_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["pod", "repo", "update"])),
            PodSubcommand::Other
        );
    }

    #[test]
    fn bare_pod_returns_other() {
        assert_eq!(detect_subcommand(&args(&["pod"])), PodSubcommand::Other);
    }
}
