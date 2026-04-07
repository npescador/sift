/// Tuist subcommands that Sift has filters for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuistSubcommand {
    Generate,
    Fetch,
    Cache,
    Edit,
    /// Any other `tuist` invocation — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> TuistSubcommand {
    match args.get(1).map(|s| s.as_str()) {
        Some("generate") => TuistSubcommand::Generate,
        Some("fetch") => TuistSubcommand::Fetch,
        Some("cache") => TuistSubcommand::Cache,
        Some("edit") => TuistSubcommand::Edit,
        _ => TuistSubcommand::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn detects_generate() {
        assert_eq!(
            detect_subcommand(&args(&["tuist", "generate"])),
            TuistSubcommand::Generate
        );
    }

    #[test]
    fn detects_fetch() {
        assert_eq!(
            detect_subcommand(&args(&["tuist", "fetch"])),
            TuistSubcommand::Fetch
        );
    }

    #[test]
    fn detects_cache() {
        assert_eq!(
            detect_subcommand(&args(&["tuist", "cache"])),
            TuistSubcommand::Cache
        );
    }

    #[test]
    fn detects_edit() {
        assert_eq!(
            detect_subcommand(&args(&["tuist", "edit"])),
            TuistSubcommand::Edit
        );
    }

    #[test]
    fn other_returns_other() {
        assert_eq!(
            detect_subcommand(&args(&["tuist", "clean"])),
            TuistSubcommand::Other
        );
    }

    #[test]
    fn bare_tuist_returns_other() {
        assert_eq!(detect_subcommand(&args(&["tuist"])), TuistSubcommand::Other);
    }
}
