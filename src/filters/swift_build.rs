use std::collections::BTreeMap;

use crate::filters::{FilterOutput, Verbosity};

/// Filter `swift build` output (SPM).
///
/// Compact: `BUILD SUCCEEDED ✓` or `BUILD FAILED — N errors, M warnings`,
///          errors grouped by file (capped at 3 per file).
/// Verbose: same + warning details.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut warnings: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut build_failed = false;
    let mut build_complete = false;

    for line in raw.lines() {
        if line.contains(": error:") {
            let (file, message) = split_diagnostic(line);
            errors.entry(file).or_default().push(message);
        } else if line.contains(": warning:") {
            let (file, message) = split_diagnostic(line);
            warnings.entry(file).or_default().push(message);
        } else if line.contains("** BUILD FAILED **") || line.trim() == "build: ** BUILD FAILED **"
        {
            build_failed = true;
        } else if line.trim() == "Build complete!" {
            build_complete = true;
        }
    }

    let error_count: usize = errors.values().map(|v| v.len()).sum();
    let warning_count: usize = warnings.values().map(|v| v.len()).sum();

    if error_count == 0 && (build_complete || !build_failed) {
        let content = if warning_count == 0 {
            "\x1b[32mBUILD SUCCEEDED\x1b[0m ✓\n".to_string()
        } else {
            format!(
                "\x1b[32mBUILD SUCCEEDED\x1b[0m ✓  ({warning_count} warning{})\n",
                if warning_count == 1 { "" } else { "s" }
            )
        };
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    let mut out = String::new();

    out.push_str(&format!(
        "\x1b[31mBUILD FAILED\x1b[0m — {error_count} error{}, {warning_count} warning{}\n",
        if error_count == 1 { "" } else { "s" },
        if warning_count == 1 { "" } else { "s" },
    ));

    if !errors.is_empty() {
        out.push('\n');
        for (file, messages) in &errors {
            out.push_str(&format!("\x1b[1m{file}\x1b[0m\n"));
            for msg in messages.iter().take(3) {
                out.push_str(&format!("  \x1b[31merror:\x1b[0m {msg}\n"));
            }
            if messages.len() > 3 {
                out.push_str(&format!("  … and {} more\n", messages.len() - 3));
            }
        }
    }

    if verbosity == Verbosity::Verbose && !warnings.is_empty() {
        out.push('\n');
        for (file, messages) in &warnings {
            out.push_str(&format!("\x1b[1m{file}\x1b[0m\n"));
            for msg in messages.iter().take(3) {
                out.push_str(&format!("  \x1b[33mwarning:\x1b[0m {msg}\n"));
            }
            if messages.len() > 3 {
                out.push_str(&format!("  … and {} more\n", messages.len() - 3));
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

/// Split a compiler diagnostic line into `(short_file, line_msg)`.
fn split_diagnostic(line: &str) -> (String, String) {
    let marker = if line.contains(": error:") {
        ": error:"
    } else {
        ": warning:"
    };

    let (loc, rest) = match line.split_once(marker) {
        Some(pair) => pair,
        None => return (String::new(), line.to_string()),
    };

    // loc = "/path/to/File.swift:LINE:COL" — extract file only for grouping
    // Strip ":LINE:COL" from the end
    let file_path = strip_line_col(loc);
    let file_key = short_path(&file_path);
    let message = format!("{}: {}", format_loc(loc), rest.trim());
    (file_key, message)
}

/// Strip `:LINE:COL` or `:LINE` suffix, returning the bare file path.
fn strip_line_col(loc: &str) -> String {
    // Try stripping two numeric suffixes (:LINE:COL)
    let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
    match parts.as_slice() {
        [col, line, path]
            if col.chars().all(|c| c.is_ascii_digit())
                && line.chars().all(|c| c.is_ascii_digit()) =>
        {
            path.to_string()
        }
        [line, path] if line.chars().all(|c| c.is_ascii_digit()) => path.to_string(),
        _ => loc.to_string(),
    }
}

/// Format a location string to `file.swift:LINE`.
fn format_loc(loc: &str) -> String {
    let parts: Vec<&str> = loc.rsplitn(3, ':').collect();
    match parts.as_slice() {
        [_col, line, path] if line.chars().all(|c| c.is_ascii_digit()) => {
            let short = short_path(path);
            format!("{short}:{line}")
        }
        [line, path] if line.chars().all(|c| c.is_ascii_digit()) => {
            let short = short_path(path);
            format!("{short}:{line}")
        }
        _ => short_path(loc),
    }
}

fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return path.to_string();
    }
    parts[parts.len() - 3..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_SUCCESS: &str = "Build complete!\n";

    const SAMPLE_FAILURE: &str = "\
/Users/dev/MyApp/Sources/ContentView.swift:10:5: error: use of unresolved identifier 'ViewModelProtocol'
/Users/dev/MyApp/Sources/ContentView.swift:15:3: warning: result of call to 'loadView()' is unused
/Users/dev/MyApp/Sources/NetworkClient.swift:22:10: error: cannot convert value of type 'String' to expected argument type 'URL'
/Users/dev/MyApp/Sources/NetworkClient.swift:44:8: warning: variable 'response' was never mutated
build: ** BUILD FAILED **
";

    #[test]
    fn compact_success_shows_build_succeeded() {
        let out = filter(SAMPLE_SUCCESS, Verbosity::Compact);
        assert!(out.content.contains("BUILD SUCCEEDED"));
        assert!(out.content.contains('✓'));
    }

    #[test]
    fn compact_failure_shows_build_failed() {
        let out = filter(SAMPLE_FAILURE, Verbosity::Compact);
        assert!(out.content.contains("BUILD FAILED"));
        assert!(out.content.contains("2 errors"));
        assert!(out.content.contains("2 warnings"));
    }

    #[test]
    fn compact_failure_groups_errors_by_file() {
        let out = filter(SAMPLE_FAILURE, Verbosity::Compact);
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("NetworkClient.swift"));
    }

    #[test]
    fn compact_does_not_show_warning_details() {
        let out = filter(SAMPLE_FAILURE, Verbosity::Compact);
        // Warnings should not be listed in compact mode, only counted
        assert!(!out.content.contains("loadView()"));
    }

    #[test]
    fn verbose_shows_warning_details() {
        let out = filter(SAMPLE_FAILURE, Verbosity::Verbose);
        assert!(out.content.contains("loadView()") || out.content.contains("response"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_FAILURE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_FAILURE);
    }

    #[test]
    fn bytes_reduced_on_failure() {
        let out = filter(SAMPLE_FAILURE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn caps_errors_at_three_per_file() {
        let raw = "\
/Users/dev/MyApp/Sources/Foo.swift:1:1: error: err1
/Users/dev/MyApp/Sources/Foo.swift:2:1: error: err2
/Users/dev/MyApp/Sources/Foo.swift:3:1: error: err3
/Users/dev/MyApp/Sources/Foo.swift:4:1: error: err4
build: ** BUILD FAILED **
";
        let out = filter(raw, Verbosity::Compact);
        assert!(out.content.contains("… and 1 more"));
    }

    #[test]
    fn success_with_warnings_shows_count() {
        let raw = "\
/Users/dev/MyApp/Sources/Foo.swift:1:1: warning: unused variable
Build complete!
";
        let out = filter(raw, Verbosity::Compact);
        assert!(out.content.contains("BUILD SUCCEEDED"));
        assert!(out.content.contains("1 warning"));
    }
}
