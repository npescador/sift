/// Git subcommands that Sift has specialized filters for.
///
/// Any git subcommand not listed here falls back to passthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSubcommand {
    Status,
    Diff,
    Log,
    /// `git log --graph` — strip graph decoration lines, then compact.
    LogGraph,
    /// Any other git subcommand — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> GitSubcommand {
    match args.get(1).map(String::as_str) {
        Some("status") => GitSubcommand::Status,
        Some("diff") => GitSubcommand::Diff,
        Some("log") => {
            if args.iter().any(|a| a == "--graph") {
                GitSubcommand::LogGraph
            } else {
                GitSubcommand::Log
            }
        }
        _ => GitSubcommand::Other,
    }
}
