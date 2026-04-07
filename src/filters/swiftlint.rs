use std::collections::BTreeMap;

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

    let violations = parse_violations(raw);

    if violations.is_empty() {
        // No violations found — show the summary line if present, else passthrough
        let summary = extract_summary(raw);
        if let Some(s) = summary {
            let content = format!("\x1b[32m✓\x1b[0m SwiftLint: {s}\n");
            let filtered_bytes = content.len();
            return FilterOutput {
                content,
                original_bytes,
                filtered_bytes,
            };
        }
        return FilterOutput::passthrough(raw);
    }

    // Group by rule name, split by severity
    let mut errors: BTreeMap<String, Vec<Location>> = BTreeMap::new();
    let mut warnings: BTreeMap<String, Vec<Location>> = BTreeMap::new();
    let mut files: std::collections::HashSet<String> = std::collections::HashSet::new();

    for v in &violations {
        files.insert(v.file.clone());
        let loc = Location {
            file: short_path(&v.file),
            line: v.line,
        };
        if v.severity == "error" {
            errors.entry(v.rule.clone()).or_default().push(loc);
        } else {
            warnings.entry(v.rule.clone()).or_default().push(loc);
        }
    }

    let error_count: usize = errors.values().map(|v| v.len()).sum();
    let warning_count: usize = warnings.values().map(|v| v.len()).sum();
    let file_count = files.len();

    let mut out = String::new();

    // Summary header
    if error_count > 0 {
        out.push_str(&format!(
            "\x1b[31m{error_count} error{}\x1b[0m, \
             \x1b[33m{warning_count} warning{}\x1b[0m \
             across {file_count} file{}\n",
            plural(error_count),
            plural(warning_count),
            plural(file_count),
        ));
    } else {
        out.push_str(&format!(
            "\x1b[33m{warning_count} warning{}\x1b[0m \
             across {file_count} file{}\n",
            plural(warning_count),
            plural(file_count),
        ));
    }

    // Errors first
    if !errors.is_empty() {
        out.push('\n');
        for (rule, locs) in &errors {
            let count = locs.len();
            out.push_str(&format!(
                "  \x1b[31merror\x1b[0m  {rule:<40}  {count} violation{}\n",
                plural(count)
            ));
            if verbosity == Verbosity::Verbose {
                for loc in locs.iter().take(3) {
                    out.push_str(&format!("    {}:{}\n", loc.file, loc.line));
                }
                if locs.len() > 3 {
                    out.push_str(&format!("    … and {} more\n", locs.len() - 3));
                }
            }
        }
    }

    // Warnings
    if !warnings.is_empty() {
        out.push('\n');
        for (rule, locs) in &warnings {
            let count = locs.len();
            out.push_str(&format!(
                "  \x1b[33mwarn\x1b[0m   {rule:<40}  {count} violation{}\n",
                plural(count)
            ));
            if verbosity == Verbosity::Verbose {
                for loc in locs.iter().take(3) {
                    out.push_str(&format!("    {}:{}\n", loc.file, loc.line));
                }
                if locs.len() > 3 {
                    out.push_str(&format!("    … and {} more\n", locs.len() - 3));
                }
            }
        }
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

// ── Data ──────────────────────────────────────────────────────────────────────

struct Violation {
    file: String,
    line: u32,
    severity: String,
    rule: String,
}

struct Location {
    file: String,
    line: u32,
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse SwiftLint violation lines.
///
/// Format: `/path/file.swift:LINE:COL: SEVERITY: RULE_IDENTIFIER: message`
fn parse_violations(raw: &str) -> Vec<Violation> {
    let mut violations = Vec::new();

    for line in raw.lines() {
        // Must contain ": warning:" or ": error:" to be a violation line
        let severity = if line.contains(": warning:") {
            "warning"
        } else if line.contains(": error:") {
            "error"
        } else {
            continue;
        };

        // Split on the first ": warning:" or ": error:"
        let marker = if severity == "warning" {
            ": warning:"
        } else {
            ": error:"
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

        // rest: " RULE_IDENTIFIER: message"
        // SwiftLint puts the rule identifier before the first ": "
        let rule = rest
            .trim()
            .split_once(':')
            .map(|(r, _)| r.trim())
            .unwrap_or(rest.trim())
            .to_string();

        if rule.is_empty() {
            continue;
        }

        violations.push(Violation {
            file: file_path,
            line: line_num,
            severity: severity.to_string(),
            rule,
        });
    }

    violations
}

/// Extract the SwiftLint summary line (e.g. "Done linting! Found 0 violations, ...")
fn extract_summary(raw: &str) -> Option<String> {
    raw.lines()
        .find(|l| l.contains("Done linting") || l.contains("violations"))
        .map(|l| l.trim().to_string())
}

/// Shorten an absolute path to the last 3 components.
fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return path.to_string();
    }
    parts[parts.len() - 3..].join("/")
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
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
    fn plural_helper_works() {
        assert_eq!(plural(1), "");
        assert_eq!(plural(0), "s");
        assert_eq!(plural(2), "s");
    }
}
