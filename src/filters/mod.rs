pub mod fastlane;
pub mod git_diff;
pub mod git_log;
pub mod git_status;
pub mod grep;
pub mod ls_xcode;
pub mod read;
pub mod swift_build;
pub mod swift_package;
pub mod swift_test;
pub mod swiftlint;
pub mod xcodebuild_archive;
pub mod xcodebuild_build;
pub mod xcodebuild_list;
pub mod xcodebuild_settings;
pub mod xcodebuild_test;
pub mod xcrun_simctl;

/// Verbosity level controlling how much output Sift retains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Maximum signal reduction — default mode.
    #[default]
    Compact,
    /// More context retained (-v).
    Verbose,
    /// Near-complete output (-vv).
    VeryVerbose,
    /// Minimal filtering (-vvv).
    Maximum,
    /// Zero filtering — identical to running the command directly (--raw).
    Raw,
}

/// The output produced by running a filter over raw command output.
#[derive(Debug)]
pub struct FilterOutput {
    /// The filtered content to print to stdout.
    pub content: String,
    /// Size of the original raw output in bytes.
    pub original_bytes: usize,
    /// Size of the filtered output in bytes.
    pub filtered_bytes: usize,
}

impl FilterOutput {
    /// Percentage of bytes saved vs the original output.
    #[allow(dead_code)] // Phase 10: used by tracking stats display
    pub fn savings_percent(&self) -> f64 {
        if self.original_bytes == 0 {
            return 0.0;
        }
        let saved = self.original_bytes.saturating_sub(self.filtered_bytes);
        saved as f64 / self.original_bytes as f64 * 100.0
    }

    /// Passthrough: wrap raw output with zero filtering applied.
    pub fn passthrough(raw: &str) -> Self {
        let bytes = raw.len();
        Self {
            content: raw.to_string(),
            original_bytes: bytes,
            filtered_bytes: bytes,
        }
    }
}
