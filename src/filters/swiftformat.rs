use crate::filters::{FilterOutput, Verbosity};

/// Filter `swiftformat` output.
///
/// Compact:
/// - Show the "SwiftFormat completed" result line directly (already compact)
/// - List only files that were formatted/changed
/// - In lint mode, show error lines (file:line:rule:message)
/// - Strip "Running SwiftFormat...", "Reading configuration", "Applying rules:" line
///
/// Verbose: same + show which rules were applied.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut changed_files: Vec<String> = Vec::new();
    let mut lint_errors: Vec<String> = Vec::new();
    let mut rules_line: Option<String> = None;
    let mut completed_line: Option<String> = None;

    let noise = [
        "Running SwiftFormat",
        "Reading configuration from",
        "warning: ",
    ];

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // The "SwiftFormat completed" summary
        if trimmed.starts_with("SwiftFormat completed") {
            completed_line = Some(trimmed.to_string());
            continue;
        }

        // Rules line
        if trimmed.starts_with("Applying rules:") {
            rules_line = Some(trimmed.to_string());
            continue;
        }

        // Skip noise
        if noise.iter().any(|n| trimmed.starts_with(n)) {
            continue;
        }

        // Lint mode error lines: "path:LINE: error: (rule) message"
        if trimmed.contains(": error:") && trimmed.contains('(') {
            lint_errors.push(format_lint_error(trimmed));
            continue;
        }

        // File lines — changed files (plain paths or paths with "(N changes)")
        if trimmed.starts_with('/') || trimmed.starts_with('.') {
            let file = short_path(trimmed.split_whitespace().next().unwrap_or(trimmed));
            changed_files.push(file);
        }
    }

    let mut out = String::new();

    // Completed line (already compact, show directly)
    if let Some(ref comp) = completed_line {
        let colored = if comp.contains("error") || comp.contains("FAILED") {
            format!("\x1b[31m{comp}\x1b[0m\n")
        } else {
            format!("\x1b[32m{comp}\x1b[0m\n")
        };
        out.push_str(&colored);
    }

    // Lint errors
    if !lint_errors.is_empty() {
        out.push('\n');
        for e in &lint_errors {
            out.push_str(&format!("  \x1b[31merror:\x1b[0m {e}\n"));
        }
    }

    // Changed files
    if !changed_files.is_empty() {
        out.push('\n');
        for f in &changed_files {
            out.push_str(&format!("  {f}\n"));
        }
    }

    // Rules (verbose only)
    if verbosity == Verbosity::Verbose {
        if let Some(ref rules) = rules_line {
            out.push('\n');
            out.push_str(&format!("{rules}\n"));
        }
    }

    if out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

/// Format a lint error line to a shorter form.
fn format_lint_error(line: &str) -> String {
    // "/path/File.swift:42: error: (consecutiveBlankLines) consecutive blank lines"
    if let Some((_loc, rest)) = line.split_once(": error:") {
        let loc = short_path(_loc.trim());
        return format!("{loc}: {}", rest.trim());
    }
    short_path(line)
}

fn short_path(path: &str) -> String {
    super::util::short_path(path, 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_FORMAT: &str = "\
Running SwiftFormat...
Reading configuration from /Users/dev/MyApp/.swiftformat
Applying rules: blankLinesAroundMark, blankLinesAtEndOfScope, consecutiveBlankLines
/Users/dev/MyApp/Sources/ContentView.swift
/Users/dev/MyApp/Sources/NetworkClient.swift
/Users/dev/MyApp/Sources/PaymentService.swift
SwiftFormat completed. 3/47 files formatted, 44 skipped (0.342 seconds)
";

    const SAMPLE_DRY_RUN: &str = "\
Running SwiftFormat...
/Users/dev/MyApp/Sources/ContentView.swift (2 changes)
/Users/dev/MyApp/Sources/NetworkClient.swift (1 change)
SwiftFormat completed. 2/47 files would be formatted, 45 skipped (0.298 seconds)
";

    const SAMPLE_LINT: &str = "\
/Users/dev/MyApp/Sources/ContentView.swift:42: error: (consecutiveBlankLines) consecutive blank lines
/Users/dev/MyApp/Sources/NetworkClient.swift:15: error: (trailingCommas) add trailing comma
SwiftFormat completed. 2 errors (lint mode). (0.412 seconds)
";

    #[test]
    fn compact_shows_completed_line() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Compact);
        assert!(out.content.contains("SwiftFormat completed"));
        assert!(out.content.contains("3/47"));
    }

    #[test]
    fn compact_lists_changed_files() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Compact);
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("NetworkClient.swift"));
    }

    #[test]
    fn compact_strips_noise() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Compact);
        assert!(!out.content.contains("Running SwiftFormat"));
        assert!(!out.content.contains("Reading configuration"));
    }

    #[test]
    fn compact_lint_shows_errors() {
        let out = filter(SAMPLE_LINT, Verbosity::Compact);
        assert!(out.content.contains("consecutiveBlankLines"));
        assert!(out.content.contains("trailingCommas"));
    }

    #[test]
    fn verbose_shows_rules() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Verbose);
        assert!(out.content.contains("Applying rules:"));
        assert!(out.content.contains("blankLinesAroundMark"));
    }

    #[test]
    fn compact_does_not_show_rules() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Compact);
        assert!(!out.content.contains("Applying rules:"));
    }

    #[test]
    fn dry_run_shows_files() {
        let out = filter(SAMPLE_DRY_RUN, Verbosity::Compact);
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("NetworkClient.swift"));
        assert!(out.content.contains("would be formatted"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_FORMAT, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_FORMAT);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_FORMAT, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}
