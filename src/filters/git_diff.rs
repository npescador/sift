#![allow(dead_code)] // Stub: implemented in Phase 5

use crate::filters::{FilterOutput, Verbosity};

/// Filter `git diff` output into a compact per-file summary.
///
/// Shows per-file stats (+lines / -lines) and useful hunk headers.
/// Skips raw diff content unless verbosity >= VeryVerbose.
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 5: real implementation
    FilterOutput::passthrough(raw)
}
