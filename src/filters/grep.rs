#![allow(dead_code)] // Stub: implemented in Phase 6

use crate::filters::{FilterOutput, Verbosity};

/// Filter `grep` / `rg` output — group by file, deduplicate, cap results.
///
/// Groups matches by file with match counts.
/// Caps total output and adds a truncation notice when the cap is hit.
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 6: real implementation
    FilterOutput::passthrough(raw)
}
