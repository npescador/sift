use std::collections::BTreeMap;

use crate::filters::types::{Severity, SwiftlintResult, SwiftlintRuleGroup};
use crate::filters::{FilterOutput, Verbosity};

/// Filter `swiftlint` / `swiftlint lint` output.
///
/// Compact: violations grouped by rule name, error count first, then warnings.
///          Summary line: "X errors, Y warnings across Z files".
/// Verbose: same grouping but includes top-3 file locations per rule.
/// VeryVerbose+: raw passthrough.
///
/// SwiftLint violation line format:
/// `/path/to/File.swift:42:16: warning: rule_name: message`
/// `/path/to/File.swift:42:16: error: force_cast: Force casts should be avoided.`
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
    let content = render(&result, verbosity, raw);
    let filtered_bytes = content.len();
    let structured = serde_json::to_value(&result).ok();
    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured,
    }
}

/// Parse raw `swiftlint` output into a structured result.
pub fn parse(raw: &str) -> SwiftlintResult {
    let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut warnings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();

    for line in raw.lines() {
        let severity = if line.contains(": warning:") {
            Severity::Warning
        } else if line.contains(": error:") {
            Severity::Error
        } else {
            continue;
        };

        let marker = match severity {
            Severity::Warning => ": warning:",
            Severity::Error => ": error:",
        };

        let (location_part, rest) = match line.split_once(marker) {
            Some(pair) => pair,
            None => continue,
        };

        // location_part: "/path/file.swift:LINE:COL"
        let mut loc_parts = location_part.rsplitn(3, ':');
        let _col = loc_parts.next().unwrap_or("0");
        let line_num: u32 = loc_parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let file_path: String = loc_parts.next().unwrap_or("").to_string();

        if file_path.is_empty() {
            continue;
        }

        let rule = rest
            .trim()
            .split_once(':')
            .map(|(r, _)| r.trim())
            .unwrap_or(rest.trim())
            .to_string();

        if rule.is_empty() {
            continue;
        }

        files.insert(file_path.clone());
        let loc = format!("{}:{}", short_path(&file_path), line_num);

        match severity {
            Severity::Error => errors.entry(rule).or_default().push(loc),
            Severity::Warning => warnings.entry(rule).or_default().push(loc),
        }
    }

    let error_count: usize = errors.values().map(|v| v.len()).sum();
    let warning_count: usize = warnings.values().map(|v| v.len()).sum();

    let mut rules: Vec<SwiftlintRuleGroup> = Vec::new();

    // Errors first, then warnings (BTreeMap keeps rules sorted alphabetically)
    for (rule, locs) in errors {
        rules.push(SwiftlintRuleGroup {
            rule,
            severity: Severity::Error,
            count: locs.len(),
            locations: locs,
        });
    }
    for (rule, locs) in warnings {
        rules.push(SwiftlintRuleGroup {
            rule,
            severity: Severity::Warning,
            count: locs.len(),
            locations: locs,
        });
    }

    SwiftlintResult {
        total_violations: error_count + warning_count,
        error_count,
        warning_count,
        file_count: files.len(),
        rules,
    }
}

/// Render the structured result as human-readable text.
fn render(result: &SwiftlintResult, verbosity: Verbosity, raw: &str) -> String {
    if result.total_violations == 0 {
        if let Some(s) = extract_summary(raw) {
            return format!("\x1b[32m✓\x1b[0m SwiftLint: {s}\n");
        }
        return raw.to_string();
    }

    let mut out = String::new();

    // Summary header
    if result.error_count > 0 {
        out.push_str(&format!(
            "\x1b[31m{} error{}\x1b[0m, \
             \x1b[33m{} warning{}\x1b[0m \
             across {} file{}\n",
            result.error_count,
            plural(result.error_count),
            result.warning_count,
            plural(result.warning_count),
            result.file_count,
            plural(result.file_count),
        ));
    } else {
        out.push_str(&format!(
            "\x1b[33m{} warning{}\x1b[0m \
             across {} file{}\n",
            result.warning_count,
            plural(result.warning_count),
            result.file_count,
            plural(result.file_count),
        ));
    }

    // Group by severity for display
    let error_rules: Vec<_> = result
        .rules
        .iter()
        .filter(|r| r.severity == Severity::Error)
        .collect();
    let warning_rules: Vec<_> = result
        .rules
        .iter()
        .filter(|r| r.severity == Severity::Warning)
        .collect();

    if !error_rules.is_empty() {
        out.push('\n');
        for group in &error_rules {
            out.push_str(&format!(
                "  \x1b[31merror\x1b[0m  {:<40}  {} violation{}\n",
                group.rule,
                group.count,
                plural(group.count)
            ));
            if verbosity == Verbosity::Verbose {
                for loc in group.locations.iter().take(3) {
                    out.push_str(&format!("    {loc}\n"));
                }
                if group.locations.len() > 3 {
                    out.push_str(&format!(
                        "    … and {} more\n",
                        group.locations.len() - 3
                    ));
                }
            }
        }
    }

    if !warning_rules.is_empty() {
        out.push('\n');
        for group in &warning_rules {
            out.push_str(&format!(
                "  \x1b[33mwarn\x1b[0m   {:<40}  {} violation{}\n",
                group.rule,
                group.count,
                plural(group.count)
            ));
            if verbosity == Verbosity::Verbose {
                for loc in group.locations.iter().take(3) {
                    out.push_str(&format!("    {loc}\n"));
                }
                if group.locations.len() > 3 {
                    out.push_str(&format!(
                        "    … and {} more\n",
                        group.locations.len() - 3
                    ));
                }
            }
        }
    }

    out
}

/// Extract the SwiftLint summary line (e.g. "Done linting! Found 0 violations, ...")
fn extract_summary(raw: &str) -> Option<String> {
    raw.lines()
        .find(|l| l.contains("Done linting") || l.contains("violations"))
        .map(|l| l.trim().to_string())
}

fn short_path(path: &str) -> String {
    super::util::short_path(path, 3)
}

fn plural(n: usize) -> &'static str {
    super::util::plural(n)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
/Users/dev/MyApp/ContentView.swift:42:16: warning: line_length: Line should be 120 characters or less: currently 135 characters
/Users/dev/MyApp/ContentView.swift:45:8: warning: trailing_whitespace: Lines should not have trailing whitespace: current line has 1 trailing whitespace characters
/Users/dev/MyApp/LoginView.swift:15:5: warning: line_length: Line should be 120 characters or less: currently 142 characters
/Users/dev/MyApp/LoginView.swift:88:12: error: force_cast: Force casts should be avoided
/Users/dev/MyApp/PaymentService.swift:33:4: error: force_cast: Force casts should be avoided
/Users/dev/MyApp/PaymentService.swift:55:22: warning: trailing_whitespace: Lines should not have trailing whitespace: current line has 2 trailing whitespace characters
/Users/dev/MyApp/NetworkLayer.swift:12:18: warning: line_length: Line should be 120 characters or less: currently 156 characters
Done linting! Found 7 violations, 2 serious in 4 files.
";

    const SAMPLE_CLEAN: &str = "\
Done linting! Found 0 violations, 0 serious in 8 files.
";

    #[test]
    fn compact_shows_summary_header() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("2 errors"));
        assert!(out.content.contains("5 warnings"));
        assert!(out.content.contains("4 files"));
    }

    #[test]
    fn compact_groups_by_rule() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("force_cast"));
        assert!(out.content.contains("line_length"));
        assert!(out.content.contains("trailing_whitespace"));
    }

    #[test]
    fn compact_shows_violation_counts_per_rule() {
        let out = filter(SAMPLE, Verbosity::Compact);
        // force_cast has 2 violations
        assert!(out.content.contains("2 violations"));
        // line_length has 3 violations
        assert!(out.content.contains("3 violations"));
    }

    #[test]
    fn compact_strips_individual_file_paths() {
        let out = filter(SAMPLE, Verbosity::Compact);
        // Full paths should not appear in compact mode
        assert!(!out
            .content
            .contains("/Users/dev/MyApp/ContentView.swift:42"));
    }

    #[test]
    fn verbose_shows_file_locations() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        // Should show shortened file paths with line numbers
        assert!(out.content.contains("ContentView.swift") || out.content.contains("MyApp"));
    }

    #[test]
    fn errors_shown_before_warnings() {
        let out = filter(SAMPLE, Verbosity::Compact);
        let err_pos = out.content.find("force_cast").unwrap_or(usize::MAX);
        let warn_pos = out.content.find("line_length").unwrap_or(usize::MAX);
        assert!(err_pos < warn_pos, "errors should appear before warnings");
    }

    #[test]
    fn clean_run_shows_success_message() {
        let out = filter(SAMPLE_CLEAN, Verbosity::Compact);
        assert!(out.content.contains('✓'));
        assert!(out.content.contains("SwiftLint"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn parse_returns_structured_result() {
        let result = parse(SAMPLE);
        assert_eq!(result.error_count, 2);
        assert_eq!(result.warning_count, 5);
        assert_eq!(result.total_violations, 7);
        assert_eq!(result.file_count, 4);
        assert_eq!(result.rules.len(), 3); // force_cast, line_length, trailing_whitespace
    }

    #[test]
    fn parse_clean_returns_zero_violations() {
        let result = parse(SAMPLE_CLEAN);
        assert_eq!(result.total_violations, 0);
        assert!(result.rules.is_empty());
    }

    #[test]
    fn plural_helper_works() {
        assert_eq!(plural(1), "");
        assert_eq!(plural(0), "s");
        assert_eq!(plural(2), "s");
    }
}
