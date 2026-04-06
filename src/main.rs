mod cli;
mod commands;
mod config;
mod error;
mod executor;
mod filters;
mod tracking;

use anyhow::Result;
use clap::Parser;
use commands::CommandFamily;
use filters::Verbosity;

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
    let cfg = config::load();

    let verbosity = if cli.raw {
        Verbosity::Raw
    } else if cli.verbose > 0 {
        cli.verbosity()
    } else {
        config::parse_verbosity(&cfg.defaults.verbosity)
    };

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

            let filter_output = apply_filter(&args, &output.stdout, verbosity);

            if !filter_output.content.is_empty() {
                print!("{}", filter_output.content);
            }
            if !output.stderr.is_empty() {
                eprint!("{}", output.stderr);
            }

            Ok(output.exit_code)
        }
    }
}

/// Route command output through the appropriate filter.
///
/// Detects the command family from `args`, selects the filter, and applies it.
/// Unknown commands and `--raw` mode always return unmodified output.
fn apply_filter(args: &[String], stdout: &str, verbosity: Verbosity) -> filters::FilterOutput {
    if verbosity == Verbosity::Raw {
        return filters::FilterOutput::passthrough(stdout);
    }

    match commands::detect(args) {
        CommandFamily::Git(sub) => match sub {
            commands::git::GitSubcommand::Status => filters::git_status::filter(stdout, verbosity),
            commands::git::GitSubcommand::Diff => filters::git_diff::filter(stdout, verbosity),
            commands::git::GitSubcommand::Other => filters::FilterOutput::passthrough(stdout),
        },
        CommandFamily::Grep => filters::grep::filter(stdout, verbosity),
        CommandFamily::Read => filters::read::filter(stdout, verbosity),
        CommandFamily::Xcodebuild(sub) => match sub {
            commands::xcodebuild::XcodebuildSubcommand::Build => {
                filters::xcodebuild_build::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Test => {
                filters::xcodebuild_test::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Other => {
                filters::FilterOutput::passthrough(stdout)
            }
        },
        CommandFamily::Unknown => filters::FilterOutput::passthrough(stdout),
    }
}
