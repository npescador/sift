use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcresulttool get` output.
///
/// Compact: if JSON, extract testStatus/testsCount/failedTests and show a
///          `TEST PASSED/FAILED  N tests` summary similar to xcodebuild_test.
///          If not JSON or unrecognized format: passthrough.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // Only attempt to filter if it looks like JSON
    if !raw.trim_start().starts_with('{') && !raw.trim_start().starts_with('[') {
        return FilterOutput::passthrough(raw);
    }

    let test_status = extract_json_string_field(raw, "testStatus")
        .or_else(|| extract_json_string_field(raw, "status"));
    let tests_count = extract_json_number_field(raw, "testsCount")
        .or_else(|| extract_json_number_field(raw, "testCount"));
    let failed_tests = extract_json_number_field(raw, "failedTests")
        .or_else(|| extract_json_number_field(raw, "failureCount"))
        .unwrap_or(0);
    let warning_count = extract_json_number_field(raw, "warningCount").unwrap_or(0);

    let Some(status) = test_status else {
        return FilterOutput::passthrough(raw);
    };

    let total = tests_count.unwrap_or(0);
    let passed = total.saturating_sub(failed_tests);

    let mut out = String::new();

    let status_lower = status.to_lowercase();
    if status_lower.contains("pass") || status_lower.contains("success") {
        out.push_str(&format!(
            "\x1b[32mTEST PASSED\x1b[0m  {total} test{}\n",
            if total == 1 { "" } else { "s" }
        ));
    } else {
        out.push_str(&format!(
            "\x1b[31mTEST FAILED\x1b[0m  {total} test{} — \
             \x1b[32m{passed} passed\x1b[0m, \x1b[31m{failed_tests} failed\x1b[0m\n",
            if total == 1 { "" } else { "s" }
        ));
    }

    if warning_count > 0 {
        out.push_str(&format!(
            "  {warning_count} warning{}\n",
            if warning_count == 1 { "" } else { "s" }
        ));
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

/// Extract a string value from a flat JSON field: `"key": "value"`.
fn extract_json_string_field(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after_key = &json[pos + pattern.len()..];
    // Find the colon
    let (_, after_colon) = after_key.split_once(':')?;
    let after_colon = after_colon.trim_start();
    // Find the opening quote
    let inner = after_colon.strip_prefix('"')?;
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

/// Extract a numeric value from a flat JSON field: `"key": 42`.
fn extract_json_number_field(json: &str, key: &str) -> Option<usize> {
    let pattern = format!("\"{}\"", key);
    let pos = json.find(&pattern)?;
    let after_key = &json[pos + pattern.len()..];
    let (_, after_colon) = after_key.split_once(':')?;
    let trimmed = after_colon.trim_start();
    // Read digits
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_PASS_JSON: &str = r#"{
  "testStatus": "Passed",
  "testsCount": 47,
  "failedTests": 0,
  "warningCount": 2
}"#;

    const SAMPLE_FAIL_JSON: &str = r#"{
  "testStatus": "Failed",
  "testsCount": 47,
  "failedTests": 2,
  "warningCount": 5
}"#;

    const SAMPLE_NON_JSON: &str = "Some non-JSON xcresult output\n";

    #[test]
    fn compact_pass_shows_test_passed() {
        let out = filter(SAMPLE_PASS_JSON, Verbosity::Compact);
        assert!(out.content.contains("TEST PASSED"));
        assert!(out.content.contains("47 tests"));
    }

    #[test]
    fn compact_fail_shows_test_failed() {
        let out = filter(SAMPLE_FAIL_JSON, Verbosity::Compact);
        assert!(out.content.contains("TEST FAILED"));
        assert!(out.content.contains("2 failed"));
        assert!(out.content.contains("45 passed"));
    }

    #[test]
    fn compact_shows_warnings() {
        let out = filter(SAMPLE_PASS_JSON, Verbosity::Compact);
        assert!(out.content.contains("2 warnings"));
    }

    #[test]
    fn non_json_is_passthrough() {
        let out = filter(SAMPLE_NON_JSON, Verbosity::Compact);
        assert_eq!(out.content, SAMPLE_NON_JSON);
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_FAIL_JSON, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_FAIL_JSON);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_FAIL_JSON, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}
