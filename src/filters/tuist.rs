use crate::filters::types::TuistResult;
use crate::filters::{FilterOutput, Verbosity};

pub fn parse(raw: &str) -> TuistResult {
    let mut targets: Vec<String> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut result_line: Option<String> = None;

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
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("Error!")
        {
            errors.push(trimmed.to_string());
            continue;
        }
        if trimmed.starts_with("Workspace generated")
            || trimmed.starts_with("Dependencies fetched")
            || trimmed.starts_with("Successfully ")
            || trimmed.contains("generated at")
        {
            result_line = Some(trimmed.to_string());
            continue;
        }
        if trimmed.starts_with('▸') {
            targets.push(trimmed.trim_start_matches('▸').trim().to_string());
            continue;
        }
        if trimmed.starts_with("Resolving:") {
            targets.push(trimmed.to_string());
            continue;
        }
        if noise_compact.iter().any(|n| trimmed.starts_with(n)) {
            continue;
        }
    }

    let succeeded = result_line.is_some() && errors.is_empty();

    TuistResult {
        succeeded,
        targets,
        errors,
        result: result_line,
    }
}

/// Filter `tuist generate`, `tuist fetch`, and other tuist subcommands.
///
/// Compact:
/// - Show result line
/// - List targets and dependencies
/// - Show errors
/// - Strip noise
///
/// Verbose: same + intermediate steps.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);

    let mut intermediate: Vec<String> = Vec::new();
    if verbosity == Verbosity::Verbose {
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
            if noise_compact.iter().any(|n| trimmed.starts_with(n)) {
                intermediate.push(trimmed.to_string());
            }
        }
    }

    let mut out = String::new();

    if !result.errors.is_empty() {
        for e in &result.errors {
            out.push_str(&format!("\x1b[31m{e}\x1b[0m\n"));
        }
    }

    if verbosity == Verbosity::Verbose && !intermediate.is_empty() {
        for s in &intermediate {
            out.push_str(&format!("{s}\n"));
        }
        if !result.targets.is_empty() || result.result.is_some() {
            out.push('\n');
        }
    }

    if !result.targets.is_empty() {
        for t in &result.targets {
            out.push_str(&format!("  ▸ {t}\n"));
        }
    }

    if let Some(ref r) = result.result {
        if !result.targets.is_empty() {
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
        structured: serde_json::to_value(&result).ok(),
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

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(SAMPLE_GENERATE);
        assert!(result.succeeded);
        assert_eq!(result.targets.len(), 3);
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE_GENERATE, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
