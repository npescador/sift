//! Filter for `sift xccov` — parses `xcrun xccov view --report --json` output.
//!
//! Shows overall coverage percentage, files below threshold, and uncovered
//! functions. Expects the JSON format produced by `xcrun xccov view`.

use crate::filters::types::{XccovFile, XccovResult};
use crate::filters::{FilterOutput, Verbosity};

/// Default coverage threshold below which files are flagged.
pub const DEFAULT_THRESHOLD: f64 = 80.0;

pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    filter_with_threshold(raw, verbosity, DEFAULT_THRESHOLD)
}

pub fn filter_with_threshold(raw: &str, verbosity: Verbosity, threshold: f64) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut result = parse(raw);
    result.threshold = threshold;
    result
        .files_below_threshold
        .retain(|f| f.percent < threshold);
    result.files_below_threshold.sort_by(|a, b| {
        a.percent
            .partial_cmp(&b.percent)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let content = format_result(&result, verbosity);
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

pub fn parse(raw: &str) -> XccovResult {
    let mut result = XccovResult {
        threshold: DEFAULT_THRESHOLD,
        ..Default::default()
    };

    // Try JSON parsing first
    if raw.trim_start().starts_with('{') {
        parse_json(raw, &mut result);
        return result;
    }

    // Fallback: line-based parsing of `xcrun xccov view` text output
    parse_text(raw, &mut result);
    result
}

fn parse_json(raw: &str, result: &mut XccovResult) {
    // xcrun xccov view --report --json produces:
    // {
    //   "coveredLines": N, "executableLines": N, "lineCoverage": 0.XX,
    //   "targets": [
    //     { "name": "...", "lineCoverage": 0.XX, "files": [
    //       { "path": "...", "lineCoverage": 0.XX, "coveredLines": N, "executableLines": N,
    //         "functions": [ { "name": "...", "lineCoverage": 0.XX, ... }, ... ]
    //       }
    //     ]}
    //   ]
    // }
    // We use a minimal scanner (no serde_json dep in sift-lib) to extract values.

    result.overall_percent = extract_root_coverage(raw);
    result.target_count = count_targets(raw);

    // Collect file paths + coverage
    for (path, percent, exec_lines, covered) in extract_file_coverages(raw) {
        let uncovered = exec_lines.saturating_sub(covered);
        result.files_below_threshold.push(XccovFile {
            path,
            percent,
            uncovered_lines: uncovered,
        });
    }
}

fn parse_text(raw: &str, result: &mut XccovResult) {
    // Text format (xcrun xccov view):
    // MyApp.app       45.12% (1234/2738)
    //     MyFile.swift    78.50% (157/200)
    for line in raw.lines() {
        let t = line.trim();
        if let Some(pct) = extract_text_percent(t) {
            if !t.starts_with('/') && !t.contains(".swift") && !t.contains(".m") {
                // Top-level target line
                if result.overall_percent == 0.0 {
                    result.overall_percent = pct;
                    result.target_count += 1;
                }
                continue;
            }
            // File line
            let path = t.split_whitespace().next().unwrap_or("").to_string();
            if !path.is_empty() {
                let uncovered = extract_text_uncovered(t);
                result.files_below_threshold.push(XccovFile {
                    path,
                    percent: pct,
                    uncovered_lines: uncovered,
                });
            }
        }
    }
}

fn format_result(result: &XccovResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    let status = if result.overall_percent >= result.threshold {
        "✓"
    } else {
        "✗"
    };
    out.push_str(&format!(
        "Coverage: {:.1}%  {} (threshold: {:.0}%)\n",
        result.overall_percent, status, result.threshold
    ));

    if result.target_count > 0 {
        out.push_str(&format!("Targets:  {}\n", result.target_count));
    }

    if result.files_below_threshold.is_empty() {
        out.push_str("All files meet the coverage threshold.\n");
        return out;
    }

    let limit = match verbosity {
        Verbosity::Compact => 10,
        Verbosity::Verbose => 25,
        _ => usize::MAX,
    };

    out.push_str(&format!(
        "\nFiles below {:.0}% ({}):\n",
        result.threshold,
        result.files_below_threshold.len()
    ));

    for file in result.files_below_threshold.iter().take(limit) {
        let short_path = shorten_path(&file.path);
        let uncov = if file.uncovered_lines > 0 {
            format!("  ({} uncovered)", file.uncovered_lines)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "  {:5.1}%  {}{}\n",
            file.percent, short_path, uncov
        ));
    }

    if result.files_below_threshold.len() > limit {
        out.push_str(&format!(
            "  (+{} more)\n",
            result.files_below_threshold.len() - limit
        ));
    }

    out
}

fn shorten_path(path: &str) -> &str {
    // Return last two path components
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    match parts.len() {
        0 => path,
        1 => parts[0],
        n => {
            let idx = path.rfind('/').unwrap_or(0);
            let idx2 = if idx > 0 {
                path[..idx].rfind('/').unwrap_or(0)
            } else {
                0
            };
            if idx2 > 0 {
                &path[idx2 + 1..]
            } else {
                parts[n - 1]
            }
        }
    }
}

// ---------------------------------------------------------------------------
// JSON mini-parsers (no dependency on serde_json)
// ---------------------------------------------------------------------------

fn extract_root_coverage(raw: &str) -> f64 {
    // Look for "lineCoverage" : 0.XXXX at the top level (before first "targets")
    let targets_pos = raw.find("\"targets\"").unwrap_or(raw.len());
    let head = &raw[..targets_pos.min(raw.len())];
    for line in head.lines() {
        if line.contains("\"lineCoverage\"") {
            if let Some(v) = extract_json_float(line) {
                return v * 100.0;
            }
        }
    }
    0.0
}

fn count_targets(raw: &str) -> usize {
    // Each target opens with a block containing "name" at depth 2 of targets array.
    // Simpler: count occurrences of "lineCoverage" inside top-level target objects,
    // identified by looking for target-level "name" keys after "targets" starts.
    //
    // The JSON structure has targets as array of objects each with "name".
    // We count distinct "name" keys that appear directly inside targets (not inside files).
    // Approach: track brace depth from "targets" opening, count depth-1 "name" keys.

    let Some(targets_start) = raw.find("\"targets\"") else {
        return 0;
    };

    let after_targets = &raw[targets_start..];
    // Find opening '[' of targets array
    let Some(bracket) = after_targets.find('[') else {
        return 0;
    };
    let targets_body = &after_targets[bracket..];

    let mut count = 0usize;
    let mut depth = 0i32; // depth inside the targets array content

    for line in targets_body.lines() {
        let t = line.trim();
        for ch in t.chars() {
            match ch {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        // At depth 1 we're inside a target object (depth == 1 after opening {)
        if depth == 1 && t.contains("\"name\"") {
            count += 1;
        }
    }

    count
}

fn extract_file_coverages(raw: &str) -> Vec<(String, f64, usize, usize)> {
    let mut results = Vec::new();

    // Find "files" array start
    let Some(files_pos) = raw.find("\"files\"") else {
        return results;
    };

    let after_keyword = &raw[files_pos + 7..]; // skip past "files"
    let Some(bracket_rel) = after_keyword.find('[') else {
        return results;
    };

    let files_content_start = files_pos + 7 + bracket_rel + 1;

    // Parse line by line, tracking brace depth relative to files array
    let mut depth = 0i32;
    let mut current_path = String::new();
    let mut current_pct = 0.0f64;
    let mut current_exec = 0usize;
    let mut current_covered = 0usize;
    let mut in_obj = false;
    let mut in_functions = false;

    for line in raw[files_content_start..].lines() {
        let t = line.trim();

        if t.is_empty() {
            continue;
        }

        // Count braces (skip inside "functions" arrays to avoid nested depth issues)
        let open = t.chars().filter(|&c| c == '{').count() as i32;
        let close = t.chars().filter(|&c| c == '}').count() as i32;

        // Check for end of files array
        if depth == 0 && t.starts_with(']') {
            break;
        }

        // Detect "functions" sub-array to skip it
        if t.contains("\"functions\"") {
            in_functions = true;
        }
        if in_functions && t.contains(']') && depth <= 1 {
            in_functions = false;
            continue;
        }
        if in_functions {
            continue;
        }

        let prev_depth = depth;
        depth += open - close;

        // Opening of a file object: depth goes from 0 to 1
        if prev_depth == 0 && depth == 1 {
            in_obj = true;
            current_path.clear();
            current_pct = 0.0;
            current_exec = 0;
            current_covered = 0;
            continue;
        }

        // Closing of a file object: depth goes from 1 to 0
        if prev_depth == 1 && depth == 0 && in_obj {
            if !current_path.is_empty() {
                results.push((
                    current_path.clone(),
                    current_pct,
                    current_exec,
                    current_covered,
                ));
            }
            in_obj = false;
            continue;
        }

        if in_obj && depth == 1 {
            if t.contains("\"path\"") {
                current_path = extract_json_string(t).unwrap_or_default();
            } else if t.contains("\"lineCoverage\"") {
                if let Some(v) = extract_json_float(t) {
                    current_pct = v * 100.0;
                }
            } else if t.contains("\"executableLines\"") {
                if let Some(v) = extract_json_usize(t) {
                    current_exec = v;
                }
            } else if t.contains("\"coveredLines\"") {
                if let Some(v) = extract_json_usize(t) {
                    current_covered = v;
                }
            }
        }
    }

    results
}

fn extract_json_float(line: &str) -> Option<f64> {
    let colon = line.find(':')?;
    let rest = line[colon + 1..].trim().trim_end_matches(',');
    rest.parse().ok()
}

fn extract_json_usize(line: &str) -> Option<usize> {
    let colon = line.find(':')?;
    let rest = line[colon + 1..].trim().trim_end_matches(',');
    rest.parse().ok()
}

fn extract_json_string(line: &str) -> Option<String> {
    let colon = line.find(':')?;
    let rest = line[colon + 1..].trim().trim_end_matches(',');
    if rest.starts_with('"') && rest.ends_with('"') {
        Some(rest[1..rest.len() - 1].to_string())
    } else {
        None
    }
}

fn extract_text_percent(line: &str) -> Option<f64> {
    for part in line.split_whitespace() {
        if part.ends_with('%') {
            return part.trim_end_matches('%').parse().ok();
        }
    }
    None
}

fn extract_text_uncovered(line: &str) -> usize {
    // Format: 78.50% (157/200)  → 200 - 157 = 43
    if let (Some(open), Some(slash), Some(close)) =
        (line.find('('), line.find('/'), line.rfind(')'))
    {
        if open < slash && slash < close {
            let covered: usize = line[open + 1..slash].trim().parse().unwrap_or(0);
            let total: usize = line[slash + 1..close].trim().parse().unwrap_or(0);
            return total.saturating_sub(covered);
        }
    }
    0
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
  "coveredLines" : 1250,
  "executableLines" : 1800,
  "lineCoverage" : 0.6944,
  "targets" : [
    {
      "coveredLines" : 1250,
      "executableLines" : 1800,
      "lineCoverage" : 0.6944,
      "name" : "MyApp.app",
      "files" : [
        {
          "path" : "/Users/dev/MyApp/Sources/NetworkClient.swift",
          "lineCoverage" : 0.9500,
          "coveredLines" : 95,
          "executableLines" : 100,
          "functions" : []
        },
        {
          "path" : "/Users/dev/MyApp/Sources/PaymentService.swift",
          "lineCoverage" : 0.4000,
          "coveredLines" : 40,
          "executableLines" : 100,
          "functions" : []
        },
        {
          "path" : "/Users/dev/MyApp/Sources/ProfileViewController.swift",
          "lineCoverage" : 0.6200,
          "coveredLines" : 62,
          "executableLines" : 100,
          "functions" : []
        }
      ]
    }
  ]
}"#;

    #[test]
    fn parses_overall_coverage() {
        let r = parse(SAMPLE_JSON);
        assert!((r.overall_percent - 69.44).abs() < 0.1);
    }

    #[test]
    fn parses_target_count() {
        let r = parse(SAMPLE_JSON);
        assert_eq!(r.target_count, 1);
    }

    #[test]
    fn files_below_threshold_excludes_passing() {
        let mut r = parse(SAMPLE_JSON);
        r.threshold = DEFAULT_THRESHOLD;
        r.files_below_threshold
            .retain(|f| f.percent < DEFAULT_THRESHOLD);
        // PaymentService (40%) and ProfileViewController (62%) are below 80%
        assert_eq!(r.files_below_threshold.len(), 2);
    }

    #[test]
    fn compact_output_contains_coverage() {
        let out = filter(SAMPLE_JSON, Verbosity::Compact);
        assert!(out.content.contains("Coverage:"));
        assert!(out.content.contains("%"));
    }

    #[test]
    fn compact_output_shows_failing_files() {
        let out = filter(SAMPLE_JSON, Verbosity::Compact);
        assert!(out.content.contains("PaymentService"));
    }

    #[test]
    fn reduces_bytes() {
        let out = filter(SAMPLE_JSON, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_passthrough() {
        let out = filter(SAMPLE_JSON, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_JSON);
    }

    #[test]
    fn structured_is_some() {
        let out = filter(SAMPLE_JSON, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
