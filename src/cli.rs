use clap::{CommandFactory, Parser, Subcommand};

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

    /// Output structured JSON instead of human-readable text
    #[arg(long, global = true)]
    pub json: bool,

    /// Stream output progressively for long-running commands
    #[arg(long, global = true)]
    pub stream: bool,

    /// Extract Swift signatures only (types, method signatures, properties — no bodies)
    ///
    /// Only applies when reading .swift files via the Proxy subcommand.
    #[arg(long, global = true)]
    pub outline: bool,

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

        /// Clear all historical statistics
        #[arg(long)]
        reset: bool,

        /// Output statistics as JSON
        #[arg(long)]
        json: bool,

        /// Show only the last N invocations
        #[arg(long, value_name = "N")]
        last: Option<usize>,
    },

    /// Install or manage sift shell hooks and AI agent instructions
    Init {
        /// Install shell functions into ~/.zshrc / ~/.bashrc so commands
        /// like git, xcodebuild, xcrun, and swiftlint are auto-filtered
        #[arg(long)]
        shell: bool,

        /// Comma-separated list of commands to wrap (default: all).
        /// Supported: git, xcodebuild, xcrun, swiftlint
        #[arg(long, value_name = "CMDS")]
        commands: Option<String>,

        /// Create/update CLAUDE.md with sift usage instructions
        #[arg(long)]
        claude: bool,

        /// Create/update .github/copilot-instructions.md with sift instructions
        #[arg(long)]
        copilot: bool,

        /// Auto-detect Xcode project and write project-specific CLAUDE.md context
        #[arg(long)]
        xcode_project: bool,

        /// Show current installation status
        #[arg(long)]
        show: bool,

        /// Remove all sift hooks and instruction files installed by sift init
        #[arg(long)]
        uninstall: bool,

        /// Install completion script for the given shell to the standard location.
        /// Alternatively, use `sift completions <shell>` to print to stdout.
        #[arg(long, value_name = "SHELL", value_enum)]
        completions: Option<clap_complete::Shell>,
    },

    /// Generate shell completion scripts
    ///
    /// Prints a completion script to stdout. Redirect to the appropriate
    /// location for your shell:
    ///
    ///   sift completions zsh > ~/.zsh/completions/_sift
    ///   sift completions bash > /usr/local/etc/bash_completion.d/sift
    ///   sift completions fish > ~/.config/fish/completions/sift.fish
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },

    /// Run built-in filter benchmarks and show reduction percentages
    ///
    /// Runs each supported command family's filter against a realistic
    /// fixture and reports input size, output size, and bytes saved.
    Benchmark,

    /// Check for a newer version and optionally self-update
    ///
    /// Downloads the latest release from GitHub and replaces the current
    /// binary in-place. Requires `curl` to be available on $PATH.
    ///
    ///   sift update           # check and install latest
    ///   sift update --check   # check only, do not install
    Update {
        /// Only check if an update is available — do not download or install
        #[arg(long)]
        check: bool,
    },

    /// Show a compact snapshot of an iOS/macOS project
    ///
    /// Reads Podfile.lock, Package.resolved, Cartfile.resolved, and
    /// project.pbxproj to produce a high-signal summary: targets, min iOS,
    /// dependencies, and source file counts.
    ///
    ///   sift project            # analyse current directory
    ///   sift project ./MyApp    # analyse a specific path
    Project {
        /// Path to the project directory or .xcodeproj (default: current directory)
        #[arg(default_value = ".")]
        path: String,
    },

    /// Parse an Apple crash report (.crash or .ips) into a compact summary
    ///
    /// Extracts exception type, device/OS info, and the crashed thread
    /// backtrace — stripping system frames, binary images, and metadata.
    ///
    ///   sift crashlog report.crash
    ///   sift crashlog report.ips
    Crashlog {
        /// Path to the .crash or .ips file
        file: String,
    },

    /// Parse an Xcode project.pbxproj file into a compact summary
    ///
    /// Extracts targets, bundle IDs, signing configuration, build phases,
    /// and inter-target dependencies.
    ///
    ///   sift pbxproj MyApp.xcodeproj/project.pbxproj
    Pbxproj {
        /// Path to the project.pbxproj file
        file: String,
    },

    /// Parse an Info.plist or .entitlements file into a compact summary
    ///
    /// Shows bundle ID, display name, version, device families, privacy
    /// permission keys, and capabilities.
    ///
    ///   sift plutil Info.plist
    ///   sift plutil MyApp.entitlements
    Plutil {
        /// Path to the .plist or .entitlements file
        file: String,
    },

    /// Parse an Apple .mobileprovision profile into a compact summary
    ///
    /// Shows profile type, app ID, team, expiry status, entitlements,
    /// and device count without requiring Xcode or security CLI.
    ///
    ///   sift provisioning MyApp_Dev.mobileprovision
    Provisioning {
        /// Path to the .mobileprovision file
        file: String,
    },

    /// Parse xcrun xccov coverage JSON into a compact summary
    ///
    /// Shows overall coverage percentage, files below threshold, and
    /// uncovered functions. Reads from stdin or a file.
    ///
    ///   xcrun xccov view --report --json MyApp.xcresult | sift xccov
    ///   sift xccov coverage.json
    Xccov {
        /// Optional path to a .json coverage report (reads stdin if omitted)
        file: Option<String>,
        /// Coverage threshold percentage (default: 80)
        #[arg(long, default_value = "80")]
        threshold: f64,
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

    /// Return a mutable clap `Command` for completion generation.
    pub fn command() -> clap::Command {
        <Self as CommandFactory>::command()
    }
}
