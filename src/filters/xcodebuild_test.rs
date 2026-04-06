#![allow(dead_code)] // Stub: implemented in Phase 8

use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild test` output — pass/fail summary with failed test details.
///
/// Shows total passed / failed / skipped counts.
/// For each failed test: name, file, line, and failure message.
/// Strips passing test noise entirely in Compact mode.
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 8: real implementation
    FilterOutput::passthrough(raw)
}
