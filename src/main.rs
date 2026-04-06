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

            let program = &args[0];
            let rest = &args[1..];

            let output = executor::execute(program, rest).map_err(|e| anyhow::anyhow!("{e}"))?;

            // Phase 4: commands::detect() will replace passthrough with filtered output
            if !output.stdout.is_empty() {
                print!("{}", output.stdout);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }

            Ok(output.exit_code)
        }
    }
}
