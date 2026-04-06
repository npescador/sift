use std::collections::BTreeMap;

use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild build` output.
///
/// Compact: unique errors grouped by file + warning count summary.
/// Verbose: errors with context lines + per-file warning counts.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut warning_count = 0usize;
    let mut build_result = "";

    for line in raw.lines() {
        if line.contains(": error:") {
            let (file, message) = split_diagnostic(line);
            errors.entry(file).or_default().push(message);
        } else if line.contains(": warning:") {
            warning_count += 1;
        } else if line.starts_with("** BUILD FAILED **") {
            build_result = "** BUILD FAILED **";
        } else if line.starts_with("** BUILD SUCCEEDED **") {
            build_result = "** BUILD SUCCEEDED **";
        }
    }

    let total_errors: usize = errors.values().map(|v| v.len()).sum();

    if total_errors == 0 && build_result == "** BUILD SUCCEEDED **" {
        let content = format!(
            "BUILD SUCCEEDED  ({warning_count} warning{})\n",
            if warning_count == 1 { "" } else { "s" }
        );
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    let mut out = String::new();

    // Error summary header
    out.push_str(&format!(
        "\x1b[31mBUILD FAILED\x1b[0m  \
         {total_errors} error{}, {warning_count} warning{}\n\n",
        if total_errors == 1 { "" } else { "s" },
        if warning_count == 1 { "" } else { "s" },
    ));

    // Errors grouped by file
    for (file, messages) in &errors {
        out.push_str(&format!("\x1b[1m{file}\x1b[0m\n"));
        for msg in messages {
            out.push_str(&format!("  \x1b[31merror:\x1b[0m {msg}\n"));
        }
    }

    if warning_count > 0 && verbosity == Verbosity::Verbose {
        out.push_str(&format!(
            "\n{warning_count} warning{} (use -vv to see details)\n",
            if warning_count == 1 { "" } else { "s" }
        ));
    }

    if !build_result.is_empty() {
        out.push_str(&format!("\n{build_result}\n"));
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

/// Split "path/file.swift:line:col: error: message" into (file_location, message).
fn split_diagnostic(line: &str) -> (String, String) {
    // Format: /path/to/file.swift:42:15: error: use of unresolved identifier 'foo'
    if let Some(err_pos) = line.find(": error:") {
        let location = &line[..err_pos];
        let message = line[err_pos + 8..].trim();
        // Shorten absolute paths — keep last 3 components
        let short = shorten_path(location);
        return (short, message.to_string());
    }
    ("unknown".to_string(), line.to_string())
}

fn shorten_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return path.to_string();
    }
    parts[parts.len() - 3..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_BUILD_FAILED: &str = "\
CompileSwift normal arm64 /Users/dev/MyApp/ContentView.swift
/Users/dev/MyApp/ContentView.swift:42:15: error: use of unresolved identifier 'foo'
/Users/dev/MyApp/ContentView.swift:43:10: warning: result of call is unused
/Users/dev/MyApp/LoginView.swift:10:5: error: cannot convert value of type 'Int' to 'String'
** BUILD FAILED **
";

    const SAMPLE_BUILD_SUCCEEDED: &str = "\
CompileSwift normal arm64 /Users/dev/MyApp/ContentView.swift
/Users/dev/MyApp/ContentView.swift:43:10: warning: result of call is unused
** BUILD SUCCEEDED **
";

    #[test]
    fn failed_build_shows_errors_grouped_by_file() {
        let out = filter(SAMPLE_BUILD_FAILED, Verbosity::Compact);
        assert!(out.content.contains("BUILD FAILED"));
        assert!(out.content.contains("2 errors"));
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("LoginView.swift"));
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn succeeded_build_shows_compact_success() {
        let out = filter(SAMPLE_BUILD_SUCCEEDED, Verbosity::Compact);
        assert!(out.content.contains("BUILD SUCCEEDED"));
        assert!(out.content.contains("1 warning"));
        assert!(!out.content.contains("CompileSwift"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_BUILD_FAILED, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_BUILD_FAILED);
    }
}
