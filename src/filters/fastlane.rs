use crate::filters::{FilterOutput, Verbosity};

pub fn filter(input: &str, verbosity: Verbosity) -> FilterOutput {
    if matches!(verbosity, Verbosity::VeryVerbose) {
        return FilterOutput::passthrough(input);
    }

    let original_bytes = input.len();
    let lines: Vec<&str> = input.lines().collect();

    let lane = extract_lane(&lines);
    let issues = extract_issues(&lines);
    let steps = extract_steps(&lines);
    let result = extract_result(&lines);

    let mut out = String::new();

    // Header
    match &lane {
        Some(name) => out.push_str(&format!("🚀 Lane: {name}\n")),
        None => out.push_str("🚀 fastlane\n"),
    }

    // Verbose: show step progression
    if matches!(verbosity, Verbosity::Verbose) && !steps.is_empty() {
        for step in &steps {
            out.push_str(&format!("  ▸ {step}\n"));
        }
        out.push('\n');
    }

    // Warnings and errors (always shown)
    for issue in &issues {
        out.push_str(&format!("  ⚠  {issue}\n"));
    }
    if !issues.is_empty() {
        out.push('\n');
    }

    // Final result
    match &result {
        Some(r) if r.contains("completed successfully") => {
            let total = extract_total_time(&lines);
            let time_str = total.map(|t| format!("  ({t} total)")).unwrap_or_default();
            out.push_str(&format!("✅ {r}{time_str}\n"));
        }
        Some(r) if r.contains("failed") => {
            out.push_str(&format!("❌ {r}\n"));
        }
        Some(r) => {
            out.push_str(&format!("{r}\n"));
        }
        None => {}
    }

    FilterOutput {
        content: out.clone(),
        original_bytes,
        filtered_bytes: out.len(),
        structured: None,
    }
}

/// Strip the `[HH:MM:SS]: ` timestamp prefix and ANSI codes from a fastlane line.
fn strip_prefix(line: &str) -> &str {
    // "[09:30:00]: some content" → "some content"
    if line.starts_with('[') {
        if let Some(rest) = line.get(12..) {
            return rest;
        }
    }
    line
}

/// Remove ANSI escape sequences from a string.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until 'm'
            for ch in chars.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Extract the lane name from `Driving the lane 'name' 🚀`.
fn extract_lane(lines: &[&str]) -> Option<String> {
    for line in lines {
        let content = strip_prefix(line);
        if content.contains("Driving the lane") {
            if let Some(start) = content.find('\'') {
                if let Some(end) = content[start + 1..].find('\'') {
                    return Some(content[start + 1..start + 1 + end].to_string());
                }
            }
        }
    }
    None
}

/// Extract warning/error lines: `[!] ...` or lines containing `error:` (case-insensitive).
fn extract_issues(lines: &[&str]) -> Vec<String> {
    let mut issues = Vec::new();
    for line in lines {
        let content = strip_ansi(strip_prefix(line));
        let trimmed = content.trim();
        if trimmed.starts_with("[!]") || trimmed.to_lowercase().contains("error:") {
            let clean = trimmed.trim_start_matches("[!]").trim().to_string();
            if !clean.is_empty() {
                issues.push(clean);
            }
        }
    }
    issues
}

/// Extract step names from `Step 'name' (N/M) done.` lines.
fn extract_steps(lines: &[&str]) -> Vec<String> {
    let mut steps = Vec::new();
    for line in lines {
        let content = strip_prefix(line);
        if content.contains("done. ⏱") || content.contains("done. \u{23f1}") {
            if let Some(start) = content.find('\'') {
                if let Some(end) = content[start + 1..].find('\'') {
                    let name = &content[start + 1..start + 1 + end];
                    // Extract "(N/M)" part
                    let progress = content
                        .find('(')
                        .and_then(|i| content[i..].find(')').map(|j| &content[i..=i + j]))
                        .unwrap_or("");
                    steps.push(format!("{name}  {progress}"));
                }
            }
        }
    }
    steps
}

/// Extract the final result line: `Lane '...' completed successfully` or `failed`.
fn extract_result(lines: &[&str]) -> Option<String> {
    for line in lines.iter().rev() {
        let content = strip_ansi(strip_prefix(line));
        let trimmed = content.trim().to_string();
        if (trimmed.contains("Lane") && trimmed.contains("completed successfully"))
            || (trimmed.contains("Lane") && trimmed.contains("failed"))
        {
            return Some(trimmed);
        }
    }
    None
}

/// Sum up total time from the step timing table (last `| N | action | X.XX s |` rows).
fn extract_total_time(lines: &[&str]) -> Option<String> {
    let mut total_secs: f64 = 0.0;
    let mut found = false;

    for line in lines {
        let content = strip_prefix(line);
        // Table data rows look like: `| 1    | gym      | 44.23 s     |`
        if content.trim_start().starts_with('|') {
            let cols: Vec<&str> = content.split('|').collect();
            if cols.len() >= 4 {
                let time_col = cols[cols.len() - 2].trim();
                if let Some(s) = time_col.strip_suffix(" s") {
                    if let Ok(secs) = s.trim().parse::<f64>() {
                        total_secs += secs;
                        found = true;
                    }
                }
            }
        }
    }

    if found && total_secs > 0.0 {
        if total_secs >= 60.0 {
            let mins = (total_secs / 60.0) as u64;
            let secs = total_secs as u64 % 60;
            Some(format!("{mins}m {secs}s"))
        } else {
            Some(format!("{total_secs:.0}s"))
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_success() -> &'static str {
        "[09:30:00]: fastlane detected a Fastfile\n\
         [09:30:00]: Driving the lane 'ios beta' \u{1f680}\n\
         [09:30:00]: -----------------------------\n\
         [09:30:00]: --- Step: increment_build_number ---\n\
         [09:30:00]: -----------------------------\n\
         [09:30:01]: Step 'increment_build_number' (1/3) done. \u{23f1}\n\
         [09:30:01]: --- Step: gym ---\n\
         [09:30:45]: Step 'gym' (2/3) done. \u{23f1}\n\
         [09:30:45]: --- Step: pilot ---\n\
         [09:31:00]: Step 'pilot' (3/3) done. \u{23f1}\n\
         [09:31:00]: Lane 'ios beta' completed successfully \u{1f389}\n\
         [09:31:00]: | 1    | increment_build_number | 1.00 s      |\n\
         [09:31:00]: | 2    | gym                    | 44.00 s     |\n\
         [09:31:00]: | 3    | pilot                  | 15.00 s     |\n\
         [09:31:00]: fastlane.tools finished. \u{1f680}\n"
    }

    fn sample_failure() -> &'static str {
        "[09:30:00]: Driving the lane 'ios build' \u{1f680}\n\
         [09:30:00]: --- Step: gym ---\n\
         [09:30:10]: \x1b[31m[!] Error: No provisioning profile found\x1b[0m\n\
         [09:30:10]: [!] No profiles for 'com.example.app' were found\n\
         [09:30:10]: Lane 'ios build' failed \u{1f4a5}\n"
    }

    #[test]
    fn extracts_lane_name() {
        let lines: Vec<&str> = sample_success().lines().collect();
        assert_eq!(extract_lane(&lines), Some("ios beta".to_string()));
    }

    #[test]
    fn extracts_steps() {
        let lines: Vec<&str> = sample_success().lines().collect();
        let steps = extract_steps(&lines);
        assert_eq!(steps.len(), 3);
        assert!(steps[0].contains("increment_build_number"));
        assert!(steps[1].contains("gym"));
        assert!(steps[2].contains("pilot"));
    }

    #[test]
    fn extracts_result_success() {
        let lines: Vec<&str> = sample_success().lines().collect();
        let result = extract_result(&lines).unwrap();
        assert!(result.contains("completed successfully"));
    }

    #[test]
    fn extracts_result_failure() {
        let lines: Vec<&str> = sample_failure().lines().collect();
        let result = extract_result(&lines).unwrap();
        assert!(result.contains("failed"));
    }

    #[test]
    fn extracts_issues_from_failure() {
        let lines: Vec<&str> = sample_failure().lines().collect();
        let issues = extract_issues(&lines);
        assert!(!issues.is_empty());
        assert!(issues
            .iter()
            .any(|i| i.contains("provisioning") || i.contains("profiles")));
    }

    #[test]
    fn compact_success_shows_checkmark_and_time() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.content.contains("✅"));
        assert!(out.content.contains("ios beta"));
        assert!(out.content.contains("60s") || out.content.contains("1m"));
    }

    #[test]
    fn compact_failure_shows_cross() {
        let out = filter(sample_failure(), Verbosity::Compact);
        assert!(out.content.contains("❌"));
        assert!(out.content.contains("ios build"));
    }

    #[test]
    fn verbose_shows_steps() {
        let out = filter(sample_success(), Verbosity::Verbose);
        assert!(out.content.contains("▸"));
        assert!(out.content.contains("gym"));
    }

    #[test]
    fn very_verbose_is_passthrough() {
        let out = filter(sample_success(), Verbosity::VeryVerbose);
        assert_eq!(out.content, sample_success());
    }

    #[test]
    fn bytes_significantly_reduced_on_success() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn strip_ansi_removes_escape_sequences() {
        let input = "\x1b[31mhello\x1b[0m";
        assert_eq!(strip_ansi(input), "hello");
    }
}
