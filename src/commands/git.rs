/// Git subcommands that Sift has specialized filters for.
///
/// Any git subcommand not listed here falls back to passthrough.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitSubcommand {
    Status,
    Diff,
    /// Any other git subcommand — passed through unfiltered.
    Other,
}

pub fn detect_subcommand(args: &[String]) -> GitSubcommand {
    match args.get(1).map(String::as_str) {
        Some("status") => GitSubcommand::Status,
        Some("diff") => GitSubcommand::Diff,
        _ => GitSubcommand::Other,
    }
}
