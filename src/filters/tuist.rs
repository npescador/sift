use crate::filters::{FilterOutput, Verbosity};

/// Filter `tuist generate`, `tuist fetch`, and other tuist subcommands.
///
/// Compact:
/// - Show result line (last meaningful line or explicit success/error)
/// - List targets (lines starting with `  ▸`) and dependencies (`  Resolving:`)
/// - Show errors
/// - Strip "Loading package", "Resolving package dependencies",
///   "Generating workspace...", "Generating Xcode workspace..."
///
/// Verbose: same + intermediate steps.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut targets: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut result_line: Option<String> = None;
    let mut intermediate: Vec<String> = Vec::new();

    let noise_compact = [
        "Loading package at",
        "Resolving package dependencies",
        "Generating workspace...",
        "Generating Xcode workspace...",
        "Fetching dependencies...",
    ];

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Error lines
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("Error!")
        {
            errors.push(trimmed.to_string());
            continue;
        }

        // Result / completion lines
        if trimmed.starts_with("Workspace generated")
            || trimmed.starts_with("Dependencies fetched")
            || trimmed.starts_with("Successfully ")
            || trimmed.contains("generated at")
        {
            result_line = Some(trimmed.to_string());
            continue;
        }

        // Target lines: "  ▸ Target MyApp"
        if trimmed.starts_with('▸') {
            targets.push(trimmed.trim_start_matches('▸').trim().to_string());
            continue;
        }

        // Dependency lines: "  Resolving: Alamofire 5.8.1"
        if trimmed.starts_with("Resolving:") {
            targets.push(trimmed.to_string());
            continue;
        }

        // Skip noise in compact mode
        if noise_compact.iter().any(|n| trimmed.starts_with(n)) {
            if verbosity == Verbosity::Verbose {
                intermediate.push(trimmed.to_string());
            }
            continue;
        }

        // Intermediate steps (kept in verbose)
        if verbosity == Verbosity::Verbose {
            intermediate.push(trimmed.to_string());
        }
    }

    let mut out = String::new();

    // Errors first
    if !errors.is_empty() {
        for e in &errors {
            out.push_str(&format!("\x1b[31m{e}\x1b[0m\n"));
        }
    }

    // Verbose: intermediate steps
    if verbosity == Verbosity::Verbose && !intermediate.is_empty() {
        for s in &intermediate {
            out.push_str(&format!("{s}\n"));
        }
        if !targets.is_empty() || result_line.is_some() {
            out.push('\n');
        }
    }

    // Targets / dependencies
    if !targets.is_empty() {
        for t in &targets {
            out.push_str(&format!("  ▸ {t}\n"));
        }
    }

    // Result line
    if let Some(ref r) = result_line {
        if !targets.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("\x1b[32m{r}\x1b[0m\n"));
    }

    if out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GENERATE: &str = "\
Generating workspace...
Loading package at /Users/dev/MyApp
Resolving package dependencies...
Generating Xcode workspace...
  ▸ Target MyApp
  ▸ Target MyAppTests
  ▸ Target MyAppUITests
Workspace generated at MyApp.xcworkspace
";

    const SAMPLE_FETCH: &str = "\
Fetching dependencies...
  Resolving: Alamofire 5.8.1
  Resolving: KeychainAccess 4.2.2
Dependencies fetched successfully.
";

    const SAMPLE_ERROR: &str = "\
Generating workspace...
Error: Target 'MyDependency' was not found in the manifest.
";

    #[test]
    fn compact_generate_shows_targets() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Compact);
        assert!(out.content.contains("MyApp"));
        assert!(out.content.contains("MyAppTests"));
    }

    #[test]
    fn compact_generate_shows_result() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Compact);
        assert!(out.content.contains("Workspace generated"));
    }

    #[test]
    fn compact_generate_strips_noise() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Compact);
        assert!(!out.content.contains("Loading package"));
        assert!(!out.content.contains("Resolving package dependencies"));
        assert!(!out.content.contains("Generating workspace..."));
    }

    #[test]
    fn compact_fetch_shows_dependencies() {
        let out = filter(SAMPLE_FETCH, Verbosity::Compact);
        assert!(out.content.contains("Alamofire 5.8.1"));
        assert!(out.content.contains("KeychainAccess 4.2.2"));
    }

    #[test]
    fn compact_fetch_shows_result() {
        let out = filter(SAMPLE_FETCH, Verbosity::Compact);
        assert!(out.content.contains("Dependencies fetched"));
    }

    #[test]
    fn compact_error_shown() {
        let out = filter(SAMPLE_ERROR, Verbosity::Compact);
        assert!(out.content.contains("MyDependency"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_GENERATE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_GENERATE);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn verbose_shows_intermediate_steps() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Verbose);
        assert!(
            out.content.contains("Generating workspace")
                || out.content.contains("Resolving package")
        );
    }
}
