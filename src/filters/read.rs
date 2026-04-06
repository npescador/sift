#![allow(dead_code)] // Stub: implemented in Phase 7

use crate::filters::{FilterOutput, Verbosity};

/// Filter `cat` / file read output — safe truncation with line range support.
///
/// Truncates to `max_lines` (configurable). Adds a notice when truncated.
/// Binary files are detected and reported without content.
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 7: real implementation
    FilterOutput::passthrough(raw)
}
