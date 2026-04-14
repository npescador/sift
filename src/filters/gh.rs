//! Filter for `sift gh` — GitHub Actions `gh run view` / `gh run list` output.
//!
//! Strips timestamps, runner noise, and ANSI escape codes. Reuses xcodebuild
//! build/test error patterns for iOS CI log summarisation.

use crate::filters::types::{GhJob, GhRunResult};
use crate::filters::{FilterOutput, Verbosity};

pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // Detect run list vs run view
    if looks_like_run_list(raw) {
        return filter_run_list(raw, original_bytes, verbosity);
    }

    filter_run_view(raw, original_bytes, verbosity)
}

// ---------------------------------------------------------------------------
// gh run list
// ---------------------------------------------------------------------------

fn filter_run_list(raw: &str, original_bytes: usize, _verbosity: Verbosity) -> FilterOutput {
    let mut lines = Vec::new();

    for line in raw.lines() {
        let t = strip_ansi(line.trim());
        if t.is_empty() {
            continue;
        }
        // Keep STATUS WORKFLOW BRANCH COMMIT columns
        // Typical: ✓  CI  main  abc1234  2m ago
        // Filter out blank/separator lines
        lines.push(t);
    }

    let content = lines.join("\n") + "\n";
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

// ---------------------------------------------------------------------------
// gh run view
// ---------------------------------------------------------------------------

fn filter_run_view(raw: &str, original_bytes: usize, verbosity: Verbosity) -> FilterOutput {
    let result = parse_run_view(raw);
    let content = format_run_result(&result, verbosity);
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

pub fn parse_run_view(raw: &str) -> GhRunResult {
    let mut result = GhRunResult::default();
    let mut current_job: Option<GhJob> = None;
    let mut in_log = false;

    for line in raw.lines() {
        let t = strip_ansi(line.trim());
        if t.is_empty() {
            continue;
        }

        // Header: "✓ main CI · 1234567890"
        if result.workflow.is_empty() && t.contains('·') {
            let parts: Vec<&str> = t.splitn(2, '·').collect();
            if parts.len() == 2 {
                result.workflow = parts[0].trim().to_string();
            }
        }

        // Status lines
        if t.starts_with("STATUS") || t.contains("Status:") {
            if let Some(val) = extract_field_value(&t, "Status") {
                result.status = val;
            }
        }
        if t.contains("Conclusion:") {
            if let Some(val) = extract_field_value(&t, "Conclusion") {
                result.conclusion = val;
            }
        }

        // Job header: "✓ build (ubuntu-latest)"  or  "✗ test"
        if is_job_line(&t) {
            if let Some(job) = current_job.take() {
                result.jobs.push(job);
            }
            let (status, name) = parse_job_line(&t);
            current_job = Some(GhJob {
                name,
                status: status.clone(),
                conclusion: status,
                steps_failed: Vec::new(),
            });
            in_log = false;
            continue;
        }

        // Log section toggle
        if t.contains("LOGS") || t.starts_with("──") || t.starts_with("--") {
            in_log = true;
            continue;
        }

        if in_log {
            if let Some(ref mut job) = current_job {
                // Keep only error/failure lines from the log
                if is_error_line(&t) {
                    let cleaned = clean_log_line(&t);
                    if !cleaned.is_empty() {
                        job.steps_failed.push(cleaned);
                    }
                }
            }
        }
    }

    if let Some(job) = current_job {
        result.jobs.push(job);
    }

    result
}

fn format_run_result(result: &GhRunResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    if !result.workflow.is_empty() {
        out.push_str(&format!("{}\n", result.workflow));
    }
    if !result.status.is_empty() || !result.conclusion.is_empty() {
        let status = if result.conclusion.is_empty() {
            &result.status
        } else {
            &result.conclusion
        };
        out.push_str(&format!("Status: {}\n", status));
    }

    if result.jobs.is_empty() {
        return out;
    }

    let failed_jobs: Vec<&GhJob> = result
        .jobs
        .iter()
        .filter(|j| is_failed_conclusion(&j.conclusion))
        .collect();

    let passed = result.jobs.len() - failed_jobs.len();

    out.push_str(&format!(
        "\nJobs: {}  ({} passed, {} failed)\n",
        result.jobs.len(),
        passed,
        failed_jobs.len()
    ));

    if matches!(verbosity, Verbosity::Compact) {
        // Only show failed jobs
        for job in &failed_jobs {
            out.push_str(&format!("\n  ✗ {}\n", job.name));
            for step in job.steps_failed.iter().take(5) {
                out.push_str(&format!("    {}\n", step));
            }
            if job.steps_failed.len() > 5 {
                out.push_str(&format!("    (+{} more)\n", job.steps_failed.len() - 5));
            }
        }
    } else {
        for job in &result.jobs {
            let icon = if is_failed_conclusion(&job.conclusion) {
                "✗"
            } else {
                "✓"
            };
            out.push_str(&format!("\n  {} {}\n", icon, job.name));
            for step in &job.steps_failed {
                out.push_str(&format!("    {}\n", step));
            }
        }
    }

    out
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn looks_like_run_list(raw: &str) -> bool {
    // gh run list outputs a header "STATUS  TITLE  WORKFLOW..."
    raw.lines()
        .take(3)
        .any(|l| l.contains("WORKFLOW") || l.contains("BRANCH") || l.contains("EVENT"))
}

fn is_job_line(t: &str) -> bool {
    // Lines starting with ✓, ✗, ●, or similar status glyphs followed by a name
    t.starts_with('✓')
        || t.starts_with('✗')
        || t.starts_with('●')
        || t.starts_with('X')
        || (t.starts_with("* ") && !t.contains("error"))
}

fn parse_job_line(t: &str) -> (String, String) {
    let (icon, rest) = if let Some(r) = t.strip_prefix('✓') {
        ("success", r)
    } else if let Some(r) = t.strip_prefix('✗') {
        ("failure", r)
    } else if let Some(r) = t.strip_prefix('●') {
        ("skipped", r)
    } else if let Some(r) = t.strip_prefix("X ") {
        ("failure", r)
    } else {
        ("unknown", t)
    };

    // Strip "(runner)" suffix
    let name = rest.split('(').next().unwrap_or(rest).trim().to_string();

    (icon.to_string(), name)
}

fn is_failed_conclusion(s: &str) -> bool {
    matches!(
        s.to_lowercase().as_str(),
        "failure" | "failed" | "error" | "timed_out"
    )
}

fn is_error_line(t: &str) -> bool {
    let lower = t.to_lowercase();
    lower.contains("error:")
        || lower.contains("✗")
        || lower.contains("failed")
        || lower.contains("exit code")
        || lower.contains("xctest")
        || lower.contains("warning:")
}

fn clean_log_line(t: &str) -> String {
    // Remove timestamps: "2024-01-15T10:30:45.123Z " prefix
    let t = if t.len() > 25 && t.chars().nth(10) == Some('T') {
        t.split_once('Z').map(|x| x.1.trim()).unwrap_or(t)
    } else {
        t
    };
    t.to_string()
}

fn extract_field_value(line: &str, field: &str) -> Option<String> {
    let pattern = format!("{}:", field);
    let pos = line.find(&pattern)?;
    let rest = line[pos + pattern.len()..].trim();
    if rest.is_empty() {
        None
    } else {
        Some(rest.to_string())
    }
}

/// Remove ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            // Skip until 'm' or end
            for c in chars.by_ref() {
                if c == 'm' {
                    break;
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RUN_VIEW: &str = r#"✗ main CI · 9876543210
Status: completed
Conclusion: failure

✓ build (ubuntu-latest)
✗ test (ubuntu-latest)
──────────────────────────────────────────
LOGS

2024-01-15T10:30:45.123Z error: use of unresolved identifier 'MyService'
2024-01-15T10:30:46.456Z error: value of type 'String' has no member 'process'
2024-01-15T10:31:00.000Z Build failed with exit code 65
"#;

    const SAMPLE_RUN_LIST: &str = r#"STATUS  TITLE       WORKFLOW  BRANCH  EVENT   ID          ELAPSED  AGE
✓       Update deps  CI        main    push    9876543210  1m23s    2h
✗       Fix crash    CI        main    push    9876543211  2m10s    3h
"#;

    #[test]
    fn parses_job_names() {
        let r = parse_run_view(SAMPLE_RUN_VIEW);
        let names: Vec<&str> = r.jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"build"));
        assert!(names.contains(&"test"));
    }

    #[test]
    fn parses_conclusion() {
        let r = parse_run_view(SAMPLE_RUN_VIEW);
        assert_eq!(r.conclusion, "failure");
    }

    #[test]
    fn compact_output_shows_failed_job() {
        let out = filter(SAMPLE_RUN_VIEW, Verbosity::Compact);
        assert!(out.content.contains("test"));
    }

    #[test]
    fn reduces_bytes_run_view() {
        let out = filter(SAMPLE_RUN_VIEW, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn run_list_passthrough_clean() {
        let out = filter(SAMPLE_RUN_LIST, Verbosity::Compact);
        assert!(out.content.contains("✓"));
        assert!(out.content.contains("✗"));
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE_RUN_VIEW, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_RUN_VIEW);
    }

    #[test]
    fn strip_ansi_clears_escape_codes() {
        let input = "\x1b[32m✓ build\x1b[0m";
        assert_eq!(strip_ansi(input), "✓ build");
    }
}
