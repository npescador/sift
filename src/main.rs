mod cli;
mod commands;
mod config;
mod error;
mod executor;
mod filters;
mod init;
mod tee;
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
        cli::SiftCommand::Init {
            shell,
            claude,
            copilot,
            xcode_project,
            show,
            uninstall,
        } => {
            init::run(init::InitOptions {
                shell,
                claude,
                copilot,
                xcode_project,
                show,
                uninstall,
            })?;
            Ok(0)
        }
        cli::SiftCommand::Stats { all: _ } => {
            let stats = tracking::StatsFile::load();
            let summary = stats.summary();

            if summary.total == 0 {
                println!("No sift invocations recorded yet.");
                println!("Run `sift <command>` to start tracking.");
                return Ok(0);
            }

            println!("Sift Statistics");
            println!("{}", "─".repeat(41));
            println!("  Invocations:    {}", summary.total);
            println!(
                "  Original bytes: {}",
                format_bytes(summary.total_original_bytes)
            );
            println!(
                "  Filtered bytes: {}",
                format_bytes(summary.total_filtered_bytes)
            );
            println!(
                "  Bytes saved:    {}  ({:.1}% avg)",
                format_bytes(summary.savings_bytes()),
                summary.savings_percent()
            );

            if !summary.by_family.is_empty() {
                println!("{}", "─".repeat(41));
                println!("  By command:");
                for (family, count) in &summary.by_family {
                    println!("    {:<12} {} runs", family, count);
                }
            }

            Ok(0)
        }
        cli::SiftCommand::Proxy(args) => {
            if args.is_empty() {
                anyhow::bail!("no command specified — run `sift --help` for usage");
            }

            let program = &args[0];
            let rest = &args[1..];

            let output = executor::execute(program, rest).map_err(|e| anyhow::anyhow!("{e}"))?;

            let family = commands::detect(&args);
            let filter_output = apply_filter(&args, &output.stdout, verbosity);

            if cfg.tracking.enabled {
                tracking::StatsFile::append(tracking::TrackingRecord::new(
                    family.name(),
                    filter_output.original_bytes,
                    filter_output.filtered_bytes,
                    output.exit_code,
                    output.duration_ms,
                ));
            }

            // Tee mode: if the filter produced nothing from non-empty input,
            // fall back to raw output and optionally save the raw to disk.
            let content = if filter_output.content.is_empty()
                && !output.stdout.trim().is_empty()
                && verbosity != filters::Verbosity::Raw
                && !matches!(commands::detect(&args), commands::CommandFamily::Unknown)
            {
                if cfg.tee.enabled {
                    let cmd_label = args.join(" ");
                    if let Some(path) = tee::save_raw(&cmd_label, &output.stdout) {
                        eprintln!(
                            "[sift] filter produced empty output — raw saved to {}",
                            path.display()
                        );
                    }
                }
                &output.stdout
            } else {
                &filter_output.content
            };

            if !content.is_empty() {
                print!("{content}");
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
            commands::git::GitSubcommand::Log => filters::git_log::filter(stdout, verbosity),
            commands::git::GitSubcommand::LogGraph => {
                filters::git_log::filter_graph(stdout, verbosity)
            }
            commands::git::GitSubcommand::Other => filters::FilterOutput::passthrough(stdout),
        },
        CommandFamily::Grep => filters::grep::filter(stdout, verbosity),
        CommandFamily::Read => filters::read::filter(stdout, verbosity),
        CommandFamily::Ls => filters::ls_xcode::filter_ls(stdout, verbosity),
        CommandFamily::Find => filters::ls_xcode::filter_find(stdout, verbosity),
        CommandFamily::Curl => filters::curl::filter(stdout, verbosity),
        CommandFamily::Xcodebuild(sub) => match sub {
            commands::xcodebuild::XcodebuildSubcommand::Build => {
                filters::xcodebuild_build::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Test => {
                filters::xcodebuild_test::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::ShowBuildSettings => {
                filters::xcodebuild_settings::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Archive => {
                filters::xcodebuild_archive::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::List => {
                filters::xcodebuild_list::filter(stdout, verbosity)
            }
            commands::xcodebuild::XcodebuildSubcommand::Other => {
                filters::FilterOutput::passthrough(stdout)
            }
        },
        CommandFamily::Xcrun(sub) => match sub {
            commands::xcrun::XcrunSubcommand::SimctlList => {
                filters::xcrun_simctl::filter(stdout, verbosity)
            }
            commands::xcrun::XcrunSubcommand::SimctlBoot
            | commands::xcrun::XcrunSubcommand::SimctlInstall
            | commands::xcrun::XcrunSubcommand::SimctlLaunch
            | commands::xcrun::XcrunSubcommand::SimctlErase
            | commands::xcrun::XcrunSubcommand::SimctlDelete => {
                filters::xcrun_simctl::filter_simctl_action(stdout, verbosity)
            }
            commands::xcrun::XcrunSubcommand::Other => filters::FilterOutput::passthrough(stdout),
        },
        CommandFamily::XcResultTool => filters::xcresulttool::filter(stdout, verbosity),
        CommandFamily::DocC => filters::docc::filter(stdout, verbosity),
        CommandFamily::Swiftlint => filters::swiftlint::filter(stdout, verbosity),
        CommandFamily::Fastlane => filters::fastlane::filter(stdout, verbosity),
        CommandFamily::SwiftFormat => filters::swiftformat::filter(stdout, verbosity),
        CommandFamily::SwiftPackage(_) => filters::swift_package::filter(stdout, verbosity),
        CommandFamily::Pod(_) => filters::pod::filter(stdout, verbosity),
        CommandFamily::Tuist(_) => filters::tuist::filter(stdout, verbosity),
        CommandFamily::Codesign => filters::codesign::filter(stdout, verbosity),
        CommandFamily::Security => filters::codesign::filter_security(stdout, verbosity),
        CommandFamily::Agvtool => filters::agvtool::filter(stdout, verbosity),
        CommandFamily::XcodeSelect => filters::xcode_select::filter(stdout, verbosity),
        CommandFamily::SwiftBuild(sub) => match sub {
            commands::swift_build::SwiftBuildSubcommand::Build => {
                filters::swift_build::filter(stdout, verbosity)
            }
            commands::swift_build::SwiftBuildSubcommand::Test => {
                filters::swift_test::filter(stdout, verbosity)
            }
            commands::swift_build::SwiftBuildSubcommand::Other => {
                filters::FilterOutput::passthrough(stdout)
            }
        },
        CommandFamily::Unknown => filters::FilterOutput::passthrough(stdout),
    }
}

/// Format a byte count as a human-readable string (B / KB / MB).
fn format_bytes(bytes: usize) -> String {
    if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.1} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn format_bytes_shows_b_for_small_values() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
    }

    #[test]
    fn format_bytes_shows_kb_for_thousands() {
        assert_eq!(format_bytes(1_000), "1.0 KB");
        assert_eq!(format_bytes(50_000), "50.0 KB");
        assert_eq!(format_bytes(999_999), "1000.0 KB");
    }

    #[test]
    fn format_bytes_shows_mb_for_millions() {
        assert_eq!(format_bytes(1_000_000), "1.0 MB");
        assert_eq!(format_bytes(2_500_000), "2.5 MB");
    }
}
