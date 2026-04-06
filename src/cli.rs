use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "sift",
    version,
    about = "Smart output reduction for AI-assisted coding workflows",
    long_about = "Sift proxies shell commands and returns compact, high-signal summaries.\n\
                  It preserves exit codes, supports raw passthrough, and tracks token savings."
)]
pub struct Cli {
    /// Increase verbosity: -v (verbose), -vv (very verbose), -vvv (maximum)
    #[arg(short = 'v', action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Pass through raw output without any filtering
    #[arg(long, global = true)]
    pub raw: bool,

    #[command(subcommand)]
    pub command: SiftCommand,
}

#[derive(Subcommand, Debug)]
pub enum SiftCommand {
    /// Show output reduction statistics for this session
    Stats {
        /// Include all historical statistics
        #[arg(long)]
        all: bool,
    },

    /// Run a command with smart output filtering
    #[command(external_subcommand)]
    Proxy(Vec<String>),
}

impl Cli {
    /// Map CLI flags to a `Verbosity` level.
    ///
    /// `--raw` always takes precedence over `-v` flags.
    pub fn verbosity(&self) -> crate::filters::Verbosity {
        if self.raw {
            return crate::filters::Verbosity::Raw;
        }
        match self.verbose {
            0 => crate::filters::Verbosity::Compact,
            1 => crate::filters::Verbosity::Verbose,
            2 => crate::filters::Verbosity::VeryVerbose,
            _ => crate::filters::Verbosity::Maximum,
        }
    }
}
