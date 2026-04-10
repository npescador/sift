use crate::filters::types::{TestFailure, XcodebuildTestResult};
use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild test` output — pass/fail summary with failed test details.
///
/// Compact: counts (passed/failed/skipped) + each failed test name + error message.
/// Verbose: adds file location for each failure.
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

/// Parse raw `xcodebuild test` output into a structured result.
pub fn parse(raw: &str) -> XcodebuildTestResult {
    let mut passed = 0usize;
    let mut failed = 0usize;
    let skipped = 0usize;
    let mut failures: Vec<TestFailure> = Vec::new();
    let mut current_failure: Option<TestFailure> = None;
    let mut succeeded = false;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.contains("' passed (") {
            if let Some(f) = current_failure.take() {
                failures.push(f);
            }
            passed += 1;
        } else if trimmed.contains("' failed (") {
            if let Some(f) = current_failure.take() {
                failures.push(f);
            }
            failed += 1;
            let name = extract_test_name(trimmed);
            current_failure = Some(TestFailure {
                name,
                location: String::new(),
                message: String::new(),
            });
        } else if trimmed.starts_with("** TEST FAILED **") {
            succeeded = false;
            if let Some(f) = current_failure.take() {
                failures.push(f);
            }
        } else if trimmed.starts_with("** TEST SUCCEEDED **") {
            succeeded = true;
        } else if trimmed.contains(": error: XCTAssert") || trimmed.contains(": XCTAssert") {
            if let Some(ref mut f) = current_failure {
                if f.message.is_empty() {
                    let msg = trimmed
                        .split(": error:")
                        .nth(1)
                        .or_else(|| trimmed.split_once(':').map(|x| x.1))
                        .unwrap_or(trimmed)
                        .trim();
                    f.message = msg.to_string();
                    if let Some(loc) = trimmed.split(": error:").next() {
                        f.location = loc.trim().to_string();
                    }
                }
            }
        }
    }

    if let Some(f) = current_failure.take() {
        failures.push(f);
    }

    XcodebuildTestResult {
        succeeded,
        passed,
        failed,
        skipped,
        failures,
    }
}

/// Render the structured result as human-readable text.
fn render(result: &XcodebuildTestResult, verbosity: Verbosity) -> String {
    let total = result.passed + result.failed + result.skipped;
    let test_result = if result.succeeded {
        "TEST SUCCEEDED"
    } else {
        "TEST FAILED"
    };

    let mut out = String::new();

    let result_color = if result.failed > 0 {
        "\x1b[31m"
    } else {
        "\x1b[32m"
    };
    out.push_str(&format!(
        "{result_color}{test_result}\x1b[0m  \
         {total} tests — \x1b[32m{} passed\x1b[0m",
        result.passed,
    ));
    if result.failed > 0 {
        out.push_str(&format!(
            ", \x1b[31m{} failed\x1b[0m",
            result.failed
        ));
    }
    if result.skipped > 0 {
        out.push_str(&format!(", {} skipped", result.skipped));
    }
    out.push('\n');

    if !result.failures.is_empty() {
        out.push('\n');
        for f in &result.failures {
            out.push_str(&format!("  \x1b[31m✗\x1b[0m {}\n", f.name));
            if !f.message.is_empty() {
                out.push_str(&format!("    {}\n", f.message));
            }
            if verbosity == Verbosity::Verbose && !f.location.is_empty() {
                out.push_str(&format!("    at {}\n", shorten_path(&f.location)));
            }
        }
    }

    out
}

/// Extract test name from a line like:
/// `Test Case '-[MyAppTests testLogin]' failed (0.123 seconds)`
fn extract_test_name(line: &str) -> String {
    line.split('\'').nth(1).unwrap_or(line).to_string()
}

fn shorten_path(path: &str) -> String {
    super::util::short_path(path, 3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TEST_FAILED: &str = "\
Test Suite 'MyAppTests' started at 2026-04-06 10:00:00.000
Test Case '-[MyAppTests testLogin]' passed (0.123 seconds)
Test Case '-[MyAppTests testPayment]' failed (0.456 seconds)
    /Users/dev/MyAppTests/PaymentTests.swift:55: error: XCTAssertEqual failed: (\"200\") is not equal to (\"401\")
Test Case '-[MyAppTests testLogout]' passed (0.050 seconds)
** TEST FAILED **
";

    const SAMPLE_TEST_SUCCEEDED: &str = "\
Test Suite 'MyAppTests' started at 2026-04-06 10:00:00.000
Test Case '-[MyAppTests testLogin]' passed (0.123 seconds)
Test Case '-[MyAppTests testLogout]' passed (0.050 seconds)
** TEST SUCCEEDED **
";

    #[test]
    fn failed_shows_counts_and_failure_details() {
        let out = filter(SAMPLE_TEST_FAILED, Verbosity::Compact);
        assert!(out.content.contains("TEST FAILED"));
        assert!(out.content.contains("2 passed"));
        assert!(out.content.contains("1 failed"));
        assert!(out.content.contains("testPayment"));
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn succeeded_shows_compact_pass() {
        let out = filter(SAMPLE_TEST_SUCCEEDED, Verbosity::Compact);
        assert!(out.content.contains("TEST SUCCEEDED"));
        assert!(out.content.contains("2 passed"));
        assert!(!out.content.contains("Test Case"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_TEST_FAILED, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_TEST_FAILED);
    }

    #[test]
    fn extract_test_name_parses_correctly() {
        let line = "Test Case '-[MyAppTests testLogin]' failed (0.123 seconds)";
        assert_eq!(extract_test_name(line), "-[MyAppTests testLogin]");
    }
}
