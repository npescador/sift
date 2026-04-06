#![allow(dead_code)] // Stub: implemented in Phase 5

use crate::filters::{FilterOutput, Verbosity};

/// Filter `git status` output into a compact grouped summary.
///
/// Groups files by state (staged / modified / untracked) with counts.
/// Example compact output:
/// ```text
/// staged:    2 files
/// modified:  3 files  (src/main.rs, src/cli.rs, +1 more)
/// untracked: 1 file   (notes.txt)
/// ```
pub fn filter(raw: &str, _verbosity: Verbosity) -> FilterOutput {
    // Phase 5: real implementation
    FilterOutput::passthrough(raw)
}
