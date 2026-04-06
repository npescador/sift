mod cli;
mod commands;
mod config;
mod error;
mod executor;
mod filters;
mod tracking;

use anyhow::Result;
use clap::Parser;

fn main() {
    let exit_code = match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("[sift error] {e:#}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run() -> Result<i32> {
    let cli = cli::Cli::parse();
    let _verbosity = cli.verbosity();

    match cli.command {
        cli::SiftCommand::Stats { all: _ } => {
            // Phase 10: real implementation via tracking::Tracker
            println!("sift stats: tracking not yet implemented");
            Ok(0)
        }
        cli::SiftCommand::Proxy(args) => {
            if args.is_empty() {
                anyhow::bail!("no command specified — run `sift --help` for usage");
            }
            // Phase 3: executor::execute(&args) will run and capture output
            // Phase 4: commands::detect(&args) will route to the right filter
            println!("sift: {:?}", args);
            Ok(0)
        }
    }
}
