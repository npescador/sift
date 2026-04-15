//! Filter for `sift xclogparser` — parses Xcode build activity log output.
//!
//! Accepts two input formats:
//! 1. Text output from the `xclogparser` CLI tool (`xclogparser parse --reporter issues`)
//! 2. Raw `.xcactivitylog` text content (SLF0 format — binary gzip; readable text segments
//!    extracted heuristically for environments without the xclogparser tool installed)
//!
//! Shows errors, warnings, build phase times, and the slowest files to compile.

use crate::filters::{FilterOutput, Verbosity};

pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
    let content = format_result(&result, verbosity);
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

#[derive(Debug, Default)]
pub struct XclogResult {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub build_phases: Vec<(String, f64)>,
    pub slow_files: Vec<(String, f64)>,
    pub targets_built: usize,
    pub total_duration_secs: f64,
}

pub fn parse(raw: &str) -> XclogResult {
    let mut result = XclogResult::default();

    // Detect xclogparser JSON issues format
    if raw.trim_start().starts_with('{') || raw.trim_start().starts_with('[') {
        parse_xclogparser_json(raw, &mut result);
        return result;
    }

    // Line-based parsing: handles xclogparser text output and
    // heuristically extracted .xcactivitylog text segments
    parse_text(raw, &mut result);
    result
}

fn parse_xclogparser_json(raw: &str, result: &mut XclogResult) {
    // xclogparser parse --reporter issues produces JSON like:
    // { "errors": [...], "warnings": [...] }
    // Each item: { "type": "error", "detail": "...", "documentURL": "...", ... }
    for line in raw.lines() {
        let t = line.trim();
        if t.contains("\"type\" : \"error\"") || t.contains("\"type\":\"error\"") {
            // grab detail on next line or same line
            if let Some(detail) = extract_json_str_field(t, "detail") {
                result.errors.push(detail);
            }
        } else if t.contains("\"type\" : \"warning\"") || t.contains("\"type\":\"warning\"") {
            if let Some(detail) = extract_json_str_field(t, "detail") {
                result.warnings.push(detail);
            }
        } else if t.contains("\"detail\"") {
            // Continuation line with detail
        }
    }

    // If inline JSON, try to find detail values after type
    let mut last_type: Option<&str> = None;
    for line in raw.lines() {
        let t = line.trim();
        if t.contains("\"error\"") {
            last_type = Some("error");
        } else if t.contains("\"warning\"") {
            last_type = Some("warning");
        } else if t.contains("\"detail\"") {
            if let Some(detail) = extract_json_str_field(t, "detail") {
                match last_type {
                    Some("error") => {
                        if !result.errors.contains(&detail) {
                            result.errors.push(detail);
                        }
                    }
                    Some("warning") => {
                        if !result.warnings.contains(&detail) {
                            result.warnings.push(detail);
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

fn parse_text(raw: &str, result: &mut XclogResult) {
    for line in raw.lines() {
        let t = line.trim();

        if t.is_empty() {
            continue;
        }

        // Error lines: "error: ..." or "/path/file.swift:42:5: error: ..."
        if is_error_line(t) {
            let clean = clean_diagnostic(t);
            if !clean.is_empty() && !result.errors.contains(&clean) {
                result.errors.push(clean);
            }
            continue;
        }

        // Warning lines
        if is_warning_line(t) {
            let clean = clean_diagnostic(t);
            if !clean.is_empty() && !result.warnings.contains(&clean) {
                result.warnings.push(clean);
            }
            continue;
        }

        // Build phase timing: "CompileSwiftSources normal arm64 com.apple.xcode.tools.swift.compiler (in target 'MyApp' from project 'MyApp')"
        // Or from xclogparser: "Phase: Sources  Duration: 12.34s"
        if t.contains("Phase:") && t.contains("Duration:") {
            if let Some((phase, dur)) = extract_phase_timing(t) {
                result.build_phases.push((phase, dur));
            }
            continue;
        }

        // Compilation timing from xclogparser (--reporter buildTimes):
        // "12.34s  /path/to/MyFile.swift"
        if let Some((file, dur)) = extract_compile_timing(t) {
            result.slow_files.push((file, dur));
            continue;
        }

        // Target count
        if t.contains("BUILD SUCCEEDED") || t.contains("TARGETS BUILT") {
            result.targets_built += 1;
        }

        // Total duration
        if t.contains("** BUILD SUCCEEDED **") || t.contains("Build complete!") {
            if let Some(dur) = extract_duration_secs(t) {
                result.total_duration_secs = dur;
            }
        }
    }

    // Sort slow files descending
    result
        .slow_files
        .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
}

fn format_result(result: &XclogResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    // Summary line
    let error_count = result.errors.len();
    let warning_count = result.warnings.len();

    if error_count == 0 && warning_count == 0 && result.slow_files.is_empty() {
        out.push_str("Build log: no issues found\n");
        if result.total_duration_secs > 0.0 {
            out.push_str(&format!("Duration: {:.1}s\n", result.total_duration_secs));
        }
        return out;
    }

    if error_count > 0 {
        out.push_str(&format!("Errors:   {}\n", error_count));
    }
    if warning_count > 0 {
        out.push_str(&format!("Warnings: {}\n", warning_count));
    }
    if result.total_duration_secs > 0.0 {
        out.push_str(&format!("Duration: {:.1}s\n", result.total_duration_secs));
    }

    let max_errors = match verbosity {
        Verbosity::Compact => 10,
        Verbosity::Verbose => 25,
        _ => usize::MAX,
    };
    let max_warnings = match verbosity {
        Verbosity::Compact => 5,
        Verbosity::Verbose => 15,
        _ => usize::MAX,
    };

    if error_count > 0 {
        out.push_str("\nErrors:\n");
        for e in result.errors.iter().take(max_errors) {
            out.push_str(&format!("  {}\n", e));
        }
        if error_count > max_errors {
            out.push_str(&format!("  (+{} more)\n", error_count - max_errors));
        }
    }

    if warning_count > 0 && matches!(verbosity, Verbosity::Verbose) {
        out.push_str("\nWarnings:\n");
        for w in result.warnings.iter().take(max_warnings) {
            out.push_str(&format!("  {}\n", w));
        }
        if warning_count > max_warnings {
            out.push_str(&format!("  (+{} more)\n", warning_count - max_warnings));
        }
    }

    if !result.build_phases.is_empty() && matches!(verbosity, Verbosity::Verbose) {
        out.push_str("\nBuild phases:\n");
        for (phase, dur) in &result.build_phases {
            out.push_str(&format!("  {:6.1}s  {}\n", dur, phase));
        }
    }

    if !result.slow_files.is_empty() {
        let limit = match verbosity {
            Verbosity::Compact => 5,
            _ => 10,
        };
        out.push_str("\nSlowest files:\n");
        for (file, dur) in result.slow_files.iter().take(limit) {
            let short = shorten_path(file);
            out.push_str(&format!("  {:6.1}s  {}\n", dur, short));
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn is_error_line(t: &str) -> bool {
    let lower = t.to_lowercase();
    lower.contains(": error:") || t.starts_with("error:")
}

fn is_warning_line(t: &str) -> bool {
    let lower = t.to_lowercase();
    lower.contains(": warning:") || t.starts_with("warning:")
}

fn clean_diagnostic(t: &str) -> String {
    // Strip leading file path and line/column: "/path/file.swift:42:5: error: message"
    if let Some(pos) = t.find(": error:") {
        return t[pos + 2..].trim().to_string();
    }
    if let Some(pos) = t.find(": warning:") {
        return t[pos + 2..].trim().to_string();
    }
    t.to_string()
}

fn extract_phase_timing(t: &str) -> Option<(String, f64)> {
    // "Phase: Sources  Duration: 12.34s"
    let phase_start = t.find("Phase:")?.checked_add(6)?;
    let dur_start = t.find("Duration:")?.checked_add(9)?;

    let phase = t[phase_start..t.find("Duration:").unwrap_or(t.len())]
        .trim()
        .to_string();
    let dur_str = t[dur_start..].trim().trim_end_matches('s');
    let dur: f64 = dur_str.parse().ok()?;

    Some((phase, dur))
}

fn extract_compile_timing(t: &str) -> Option<(String, f64)> {
    // "  12.34s  /path/to/File.swift"
    let t = t.trim();
    if !t.ends_with(".swift") && !t.ends_with(".m") && !t.ends_with(".mm") {
        return None;
    }
    let parts: Vec<&str> = t.splitn(2, "  ").collect();
    if parts.len() < 2 {
        return None;
    }
    let dur: f64 = parts[0].trim().trim_end_matches('s').parse().ok()?;
    Some((parts[1].trim().to_string(), dur))
}

fn extract_duration_secs(t: &str) -> Option<f64> {
    // Look for patterns like "(12.345 seconds)" or "12.34s"
    if let Some(open) = t.rfind('(') {
        let inner = t[open + 1..].trim_end_matches(')');
        let parts: Vec<&str> = inner.split_whitespace().collect();
        if parts.len() == 2 && parts[1].starts_with("second") {
            return parts[0].parse().ok();
        }
    }
    None
}

fn extract_json_str_field(line: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\"", field);
    let pos = line.find(&pattern)?;
    let after = line[pos + pattern.len()..].trim();
    let after = after.strip_prefix(':')?.trim();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    Some(after[..end].to_string())
}

fn shorten_path(path: &str) -> &str {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => path,
        1 => parts[0],
        _ => {
            let idx = path.rfind('/').unwrap_or(0);
            let idx2 = if idx > 0 {
                path[..idx].rfind('/').unwrap_or(0)
            } else {
                0
            };
            if idx2 > 0 {
                &path[idx2 + 1..]
            } else {
                parts[parts.len() - 1]
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TEXT: &str = r#"Build session started at 2024-01-15 10:30:00
Compiling 47 Swift source files...

/Users/dev/MyApp/Sources/PaymentService.swift:42:5: error: use of unresolved identifier 'PaymentResult'
/Users/dev/MyApp/Sources/PaymentService.swift:67:12: error: value of type 'String' has no member 'processPayment'
/Users/dev/MyApp/Sources/NetworkClient.swift:15:8: warning: initialization of variable 'session' was never used
/Users/dev/MyApp/Sources/NetworkClient.swift:88:3: warning: result of call to 'dataTask' is unused

Phase: Sources  Duration: 18.45s
Phase: Frameworks  Duration: 2.12s
Phase: Resources  Duration: 0.87s

  18.45s  /Users/dev/MyApp/Sources/PaymentService.swift
   9.23s  /Users/dev/MyApp/Sources/NetworkClient.swift
   4.11s  /Users/dev/MyApp/Sources/AuthViewModel.swift

** BUILD FAILED ** (22.134 seconds)
"#;

    const SAMPLE_JSON: &str = r#"[
  {
    "type" : "error",
    "detail" : "use of unresolved identifier 'PaymentResult'",
    "documentURL" : "file:///Users/dev/MyApp/Sources/PaymentService.swift",
    "startingLineNumber" : 42
  },
  {
    "type" : "warning",
    "detail" : "initialization of variable 'session' was never used",
    "documentURL" : "file:///Users/dev/MyApp/Sources/NetworkClient.swift",
    "startingLineNumber" : 15
  }
]"#;

    #[test]
    fn parses_errors_from_text() {
        let r = parse(SAMPLE_TEXT);
        assert_eq!(r.errors.len(), 2);
        assert!(r.errors[0].contains("PaymentResult"));
    }

    #[test]
    fn parses_warnings_from_text() {
        let r = parse(SAMPLE_TEXT);
        assert_eq!(r.warnings.len(), 2);
    }

    #[test]
    fn parses_slow_files() {
        let r = parse(SAMPLE_TEXT);
        assert!(!r.slow_files.is_empty());
        // sorted descending
        assert!(r.slow_files[0].1 >= r.slow_files[1].1);
    }

    #[test]
    fn parses_build_phases() {
        let r = parse(SAMPLE_TEXT);
        assert_eq!(r.build_phases.len(), 3);
        let sources = r.build_phases.iter().find(|(p, _)| p == "Sources").unwrap();
        assert!((sources.1 - 18.45).abs() < 0.01);
    }

    #[test]
    fn compact_output_shows_errors() {
        let out = filter(SAMPLE_TEXT, Verbosity::Compact);
        assert!(out.content.contains("Errors:"));
        assert!(out.content.contains("PaymentResult"));
    }

    #[test]
    fn reduces_bytes() {
        let out = filter(SAMPLE_TEXT, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE_TEXT, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_TEXT);
    }

    #[test]
    fn parses_xclogparser_json_errors() {
        let r = parse(SAMPLE_JSON);
        assert!(!r.errors.is_empty());
        assert!(r.errors[0].contains("PaymentResult"));
    }
}
