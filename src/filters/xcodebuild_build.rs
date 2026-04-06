#![allow(dead_code)] // Stub: implemented in Phase 8

use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild build` output — group errors, summarize warnings.
///
/// Groups unique compiler errors by file with line numbers.
/// Summarizes warning count without printing each one.
/// Strips progress lines (CompileSwift, PhaseScriptExecution, etc.).
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 8: real implementation
    FilterOutput::passthrough(raw)
}
