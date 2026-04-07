use crate::filters::{FilterOutput, Verbosity};

/// Filter `swift test` output (SPM).
///
/// Compact: `TEST PASSED  N tests` or `TEST FAILED  N tests — X passed, Y failed`,
///          with failed test names and XCTAssert messages.
/// Verbose: same + file:line for each failure.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<FailedTest> = Vec::new();
    let mut current_failure: Option<FailedTest> = None;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.contains("Test Case '") && trimmed.contains("' started.") {
            // Pre-create a failure entry; discard if the test passes
            if let Some(f) = current_failure.take() {
                failures.push(f);
            }
            let name = extract_test_name(trimmed);
            current_failure = Some(FailedTest {
                name,
                location: String::new(),
                message: String::new(),
            });
        } else if trimmed.contains("' passed (") {
            // Test passed — discard the pre-created entry
            current_failure = None;
            passed += 1;
        } else if trimmed.contains("' failed (") {
            // Test failed — keep the pre-created entry and count it
            failed += 1;
            if current_failure.is_none() {
                let name = extract_test_name(trimmed);
                current_failure = Some(FailedTest {
                    name,
                    location: String::new(),
                    message: String::new(),
                });
            }
        } else if trimmed.contains(": error:") && trimmed.contains("XCTAssert") {
            if let Some(ref mut f) = current_failure {
                if f.message.is_empty() {
                    let msg = trimmed
                        .split(": error:")
                        .nth(1)
                        .unwrap_or(trimmed)
                        .trim()
                        .to_string();
                    f.message = msg;
                    if verbosity == Verbosity::Verbose {
                        if let Some(loc) = trimmed.split(": error:").next() {
                            f.location = shorten_path(loc);
                        }
                    }
                }
            }
        }
    }

    if let Some(f) = current_failure.take() {
        // Only keep if we counted it as failed
        if failed > failures.len() {
            failures.push(f);
        }
    }

    let total = passed + failed;

    let mut out = String::new();

    if failed == 0 {
        out.push_str(&format!(
            "\x1b[32mTEST PASSED\x1b[0m  {total} test{}\n",
            if total == 1 { "" } else { "s" }
        ));
    } else {
        out.push_str(&format!(
            "\x1b[31mTEST FAILED\x1b[0m  {total} test{} — \
             \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed} failed\x1b[0m\n",
            if total == 1 { "" } else { "s" }
        ));

        if !failures.is_empty() {
            out.push('\n');
            for f in &failures {
                out.push_str(&format!("  \x1b[31m✗\x1b[0m {}\n", f.name));
                if !f.message.is_empty() {
                    out.push_str(&format!("    {}\n", f.message));
                }
                if verbosity == Verbosity::Verbose && !f.location.is_empty() {
                    out.push_str(&format!("    at {}\n", f.location));
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

struct FailedTest {
    name: String,
    location: String,
    message: String,
}

/// Extract the test name from a "Test Case '...' failed" line.
fn extract_test_name(line: &str) -> String {
    if let Some(start) = line.find("'-[") {
        if let Some(end) = line[start..].find(']') {
            let inner = &line[start + 2..start + end + 1];
            // "-[Module.Suite testMethod]" → "Suite.testMethod"
            let without_brackets = inner.trim_matches(|c| c == '[' || c == ']');
            if let Some((suite, test)) = without_brackets.split_once(' ') {
                let short_suite = suite.split('.').next_back().unwrap_or(suite);
                return format!("{short_suite}.{test}");
            }
            return without_brackets.to_string();
        }
    }
    line.split('\'')
        .nth(1)
        .unwrap_or(line)
        .trim_matches(|c| c == '[' || c == ']')
        .to_string()
}

/// Shorten a path+location string to last 3 path components.
fn shorten_path(loc: &str) -> String {
    // loc could be "/path/to/File.swift:LINE"
    let parts: Vec<&str> = loc.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return loc.to_string();
    }
    parts[parts.len() - 3..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PASS: &str = "\
Test Suite 'All tests' started at 2026-04-07 10:00:00.123
Test Suite 'MyPackageTests.xctest' started at 2026-04-07 10:00:00.124
Test Suite 'MyTests' started at 2026-04-07 10:00:00.124
Test Case '-[MyTests.MyTests testExample]' started.
Test Case '-[MyTests.MyTests testExample]' passed (0.001 seconds).
Test Suite 'MyTests' passed at 2026-04-07 10:00:00.126.
Test Suite 'MyPackageTests.xctest' passed at 2026-04-07 10:00:00.126.
Test Suite 'All tests' passed at 2026-04-07 10:00:00.126.
Executed 1 tests, with 0 failures (0 unexpected) in 0.001 (0.003) seconds.
";

    const SAMPLE_FAIL: &str = "\
Test Suite 'All tests' started at 2026-04-07 10:00:00.123
Test Suite 'MyPackageTests.xctest' started at 2026-04-07 10:00:00.124
Test Suite 'MyTests' started at 2026-04-07 10:00:00.124
Test Case '-[MyTests.MyTests testExample]' started.
Test Case '-[MyTests.MyTests testExample]' passed (0.001 seconds).
Test Case '-[MyTests.MyTests testFailure]' started.
/path/MyTests.swift:25: error: -[MyTests.MyTests testFailure] : XCTAssertEqual failed: (\"1\") is not equal to (\"2\")
Test Case '-[MyTests.MyTests testFailure]' failed (0.002 seconds).
Test Suite 'MyTests' failed at 2026-04-07 10:00:00.127.
Test Suite 'MyPackageTests.xctest' failed at 2026-04-07 10:00:00.127.
Test Suite 'All tests' failed at 2026-04-07 10:00:00.128.
Executed 2 tests, with 1 failure (0 unexpected) in 0.003 (0.005) seconds.
";

    #[test]
    fn compact_pass_shows_test_passed() {
        let out = filter(SAMPLE_PASS, Verbosity::Compact);
        assert!(out.content.contains("TEST PASSED"));
        assert!(out.content.contains("1 test"));
    }

    #[test]
    fn compact_fail_shows_test_failed() {
        let out = filter(SAMPLE_FAIL, Verbosity::Compact);
        assert!(out.content.contains("TEST FAILED"));
        assert!(out.content.contains("1 failed"));
        assert!(out.content.contains("1 passed") || out.content.contains("passed"));
    }

    #[test]
    fn compact_fail_shows_failed_test_name() {
        let out = filter(SAMPLE_FAIL, Verbosity::Compact);
        assert!(out.content.contains("testFailure"));
    }

    #[test]
    fn compact_fail_shows_xctassert_message() {
        let out = filter(SAMPLE_FAIL, Verbosity::Compact);
        assert!(out.content.contains("XCTAssertEqual"));
    }

    #[test]
    fn verbose_fail_shows_location() {
        let out = filter(SAMPLE_FAIL, Verbosity::Verbose);
        assert!(out.content.contains("MyTests.swift") || out.content.contains("path"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_FAIL, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_FAIL);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_FAIL, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn strips_test_suite_noise() {
        let out = filter(SAMPLE_FAIL, Verbosity::Compact);
        assert!(!out.content.contains("Test Suite"));
    }

    #[test]
    fn pass_strips_all_noise() {
        let out = filter(SAMPLE_PASS, Verbosity::Compact);
        assert!(!out.content.contains("Test Case"));
        assert!(!out.content.contains("Test Suite"));
    }
}
