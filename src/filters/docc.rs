use crate::filters::{FilterOutput, Verbosity};

/// Filter `docc convert` output.
///
/// Compact:
/// - Show "Processing N symbols..." line
/// - Show result line ("Documentation converted...")
/// - Show warnings
/// - Strip "Converting documentation...", "Resolving links..."
///
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut symbols_line: Option<String> = None;
    let mut result_line: Option<String> = None;
    let mut warnings: Vec<String> = Vec::new();

    let noise = [
        "Converting documentation...",
        "Resolving links...",
        "Writing output to",
    ];

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Symbols count line
        if trimmed.starts_with("Processing ") && trimmed.contains("symbol") {
            symbols_line = Some(trimmed.to_string());
            continue;
        }

        // Result line
        if trimmed.starts_with("Documentation converted") || trimmed.starts_with("Documentation ") {
            result_line = Some(trimmed.to_string());
            continue;
        }

        // Warning lines
        if trimmed.starts_with("warning:") {
            warnings.push(trimmed.trim_start_matches("warning:").trim().to_string());
            continue;
        }

        // Skip noise
        if noise.iter().any(|n| trimmed.starts_with(n)) {
            continue;
        }
    }

    let mut out = String::new();

    // Symbols line
    if let Some(ref s) = symbols_line {
        out.push_str(&format!("{s}\n"));
    }

    // Warnings
    if !warnings.is_empty() {
        let count = warnings.len();
        out.push_str(&format!(
            "\x1b[33m⚠️  {count} warning{}: {}\x1b[0m\n",
            if count == 1 { "" } else { "s" },
            warnings
                .iter()
                .take(3)
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    // Result line
    if let Some(ref r) = result_line {
        let colored = if r.contains("error") || r.contains("fail") {
            format!("\x1b[31m✗\x1b[0m {r}\n")
        } else {
            format!("\x1b[32m✓\x1b[0m {r}\n")
        };
        out.push_str(&colored);
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

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SUCCESS: &str = "\
Converting documentation...
Resolving links...
Processing 142 symbols...
Writing output to /Users/dev/MyApp/.docc-build/
Documentation converted successfully (2.3 seconds).
";

    const SAMPLE_WITH_WARNINGS: &str = "\
Converting documentation...
warning: 'init()' is referenced but has no documentation
warning: 'fetchData()' has no documentation
Processing 142 symbols...
Documentation converted successfully with 2 warnings (2.3 seconds).
";

    #[test]
    fn compact_shows_symbols_count() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(out.content.contains("Processing 142 symbols"));
    }

    #[test]
    fn compact_shows_result_line() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(out.content.contains("Documentation converted"));
        assert!(out.content.contains('✓'));
    }

    #[test]
    fn compact_strips_noise() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(!out.content.contains("Converting documentation"));
        assert!(!out.content.contains("Resolving links"));
    }

    #[test]
    fn compact_shows_warnings() {
        let out = filter(SAMPLE_WITH_WARNINGS, Verbosity::Compact);
        assert!(out.content.contains("⚠️") || out.content.contains("warning"));
        assert!(out.content.contains("init()") || out.content.contains("2 warnings"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_SUCCESS);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn success_shows_checkmark() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(out.content.contains('✓'));
    }
}
