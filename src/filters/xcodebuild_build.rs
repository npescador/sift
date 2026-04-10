use std::collections::BTreeMap;

use crate::filters::types::XcodebuildBuildResult;
use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild build` output.
///
/// Compact: unique errors grouped by file + warning count summary.
///   Detects: Swift/ObjC errors, linker errors, provisioning/signing errors.
/// Verbose: errors with context lines + per-file warning counts.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
    let content = render(&result, verbosity);
    let filtered_bytes = content.len();
    let structured = serde_json::to_value(&result).ok();
    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured,
    }
}

/// Parse raw `xcodebuild build` output into a structured result.
pub fn parse(raw: &str) -> XcodebuildBuildResult {
    let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut linker_errors: Vec<String> = Vec::new();
    let mut signing_errors: Vec<String> = Vec::new();
    let mut warning_count = 0usize;
    let mut succeeded = false;
    let mut has_result = false;

    for line in raw.lines() {
        if line.contains(": error:") {
            let (file, message) = split_diagnostic(line);
            errors.entry(file).or_default().push(message);
        } else if is_linker_error(line) {
            let msg = extract_linker_message(line);
            if !msg.is_empty() && !linker_errors.contains(&msg) {
                linker_errors.push(msg);
            }
        } else if is_signing_error(line) {
            let msg = extract_signing_message(line);
            if !msg.is_empty() && !signing_errors.contains(&msg) {
                signing_errors.push(msg);
            }
        } else if line.contains(": warning:") {
            warning_count += 1;
        } else if line.starts_with("** BUILD FAILED **") {
            succeeded = false;
            has_result = true;
        } else if line.starts_with("** BUILD SUCCEEDED **") {
            succeeded = true;
            has_result = true;
        }
    }

    // Flatten grouped errors into Diagnostic vec for structured output
    let diagnostics = errors
        .into_iter()
        .flat_map(|(file, messages)| {
            messages.into_iter().map(move |msg| {
                crate::filters::types::Diagnostic {
                    file: file.clone(),
                    line: None,
                    column: None,
                    severity: crate::filters::types::Severity::Error,
                    message: msg,
                }
            })
        })
        .collect();

    XcodebuildBuildResult {
        succeeded: if has_result { succeeded } else { false },
        errors: diagnostics,
        warning_count,
        linker_errors,
        signing_errors,
    }
}

/// Render the structured result as human-readable text.
fn render(result: &XcodebuildBuildResult, verbosity: Verbosity) -> String {
    let total_errors =
        result.errors.len() + result.linker_errors.len() + result.signing_errors.len();

    if total_errors == 0 && result.succeeded {
        return format!(
            "BUILD SUCCEEDED  ({} warning{})\n",
            result.warning_count,
            if result.warning_count == 1 { "" } else { "s" }
        );
    }

    let mut out = String::new();

    // Header
    out.push_str(&format!(
        "\x1b[31mBUILD FAILED\x1b[0m  \
         {total_errors} error{}, {} warning{}\n",
        if total_errors == 1 { "" } else { "s" },
        result.warning_count,
        if result.warning_count == 1 { "" } else { "s" },
    ));

    // Signing / provisioning errors — highest priority, shown first
    if !result.signing_errors.is_empty() {
        out.push('\n');
        out.push_str("🔐 Signing / Provisioning\n");
        for msg in &result.signing_errors {
            out.push_str(&format!("  {msg}\n"));
        }
    }

    // Linker errors
    if !result.linker_errors.is_empty() {
        out.push('\n');
        out.push_str("🔗 Linker\n");
        for msg in &result.linker_errors {
            out.push_str(&format!("  {msg}\n"));
        }
    }

    // Swift/ObjC compiler errors grouped by file
    // Re-group from the flat diagnostics list
    let mut by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for diag in &result.errors {
        by_file.entry(&diag.file).or_default().push(&diag.message);
    }
    if !by_file.is_empty() {
        out.push('\n');
        for (file, messages) in &by_file {
            out.push_str(&format!("\x1b[1m{file}\x1b[0m\n"));
            for msg in messages {
                out.push_str(&format!("  \x1b[31merror:\x1b[0m {msg}\n"));
            }
        }
    }

    if result.warning_count > 0 && verbosity == Verbosity::Verbose {
        out.push_str(&format!(
            "\n{} warning{} (use -vv to see details)\n",
            result.warning_count,
            if result.warning_count == 1 { "" } else { "s" }
        ));
    }

    if result.succeeded {
        out.push_str("\n** BUILD SUCCEEDED **\n");
    } else {
        out.push_str("\n** BUILD FAILED **\n");
    }

    out
}

/// Detect linker error lines: `ld: ...`, `Undefined symbols`, `clang: error: linker command failed`.
fn is_linker_error(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("ld: ")
        || t.starts_with("Undefined symbols for architecture")
        || t.starts_with("clang: error: linker command failed")
        || (t.contains("symbol(s) not found") && !t.contains(": error:"))
}

fn extract_linker_message(line: &str) -> String {
    let t = line.trim();
    // "clang: error: linker command failed with exit code 1 (use -v to see invocation)"
    // → "linker command failed with exit code 1"
    if let Some(rest) = t.strip_prefix("clang: error: ") {
        return rest.split('(').next().unwrap_or(rest).trim().to_string();
    }
    t.chars().take(120).collect()
}

/// Detect provisioning / signing error lines.
fn is_signing_error(line: &str) -> bool {
    let t = line.to_lowercase();
    t.contains("provisioning profile")
        || t.contains("code signing")
        || t.contains("codesign")
        || t.contains("no profiles for")
        || t.contains("signing certificate")
        || t.contains("development team")
        || (t.contains("error:") && (t.contains("entitlement") || t.contains("bundle identifier")))
}

fn extract_signing_message(line: &str) -> String {
    let t = line.trim();
    // Strip common prefixes
    for prefix in &["error: ", "Code Sign error: "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return rest.chars().take(120).collect();
        }
    }
    t.chars().take(120).collect()
}

/// Split "path/file.swift:line:col: error: message" into (file_location, message).
fn split_diagnostic(line: &str) -> (String, String) {
    if let Some(err_pos) = line.find(": error:") {
        let location = &line[..err_pos];
        let message = line[err_pos + 8..].trim();
        let short = shorten_path(location);
        return (short, message.to_string());
    }
    ("unknown".to_string(), line.to_string())
}

fn shorten_path(path: &str) -> String {
    super::util::short_path(path, 3)
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

    const SAMPLE_LINKER_ERROR: &str = "\
CompileSwift normal arm64 /Users/dev/MyApp/PaymentService.swift
Undefined symbols for architecture arm64:
  \"_OBJC_CLASS_$_SKPaymentQueue\", referenced from:
      _main in PaymentService.o
ld: symbol(s) not found for architecture arm64
clang: error: linker command failed with exit code 1 (use -v to see invocation)
** BUILD FAILED **
";

    const SAMPLE_SIGNING_ERROR: &str = "\
Code Sign error: No matching provisioning profile found: Your build settings specify a provisioning profile with the UUID, but no such provisioning profile was found.
error: No profiles for 'com.example.MyApp' were found
** BUILD FAILED **
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
    fn linker_errors_shown_with_link_emoji() {
        let out = filter(SAMPLE_LINKER_ERROR, Verbosity::Compact);
        assert!(out.content.contains("🔗 Linker"));
        assert!(out.content.contains("linker command failed"));
    }

    #[test]
    fn signing_errors_shown_with_key_emoji() {
        let out = filter(SAMPLE_SIGNING_ERROR, Verbosity::Compact);
        assert!(out.content.contains("🔐 Signing"));
        assert!(out.content.contains("provisioning profile"));
    }

    #[test]
    fn signing_errors_shown_before_compiler_errors() {
        let combined = format!("{SAMPLE_BUILD_FAILED}{SAMPLE_SIGNING_ERROR}");
        let out = filter(&combined, Verbosity::Compact);
        let sign_pos = out.content.find("🔐").unwrap_or(usize::MAX);
        let file_pos = out.content.find("ContentView").unwrap_or(usize::MAX);
        assert!(
            sign_pos < file_pos,
            "signing errors should appear before compiler errors"
        );
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_BUILD_FAILED, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_BUILD_FAILED);
    }
}
