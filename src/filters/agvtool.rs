use crate::filters::{FilterOutput, Verbosity};

/// Filter `agvtool` output.
///
/// Compact:
/// - `what-version`: show `Version: N`
/// - `new-version`: show `Version updated → N (M files)`
/// - Other: passthrough
///
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // `what-version` output: single number or "Current version of project X is:\n    N"
    if let Some(version) = extract_what_version(raw) {
        let content = format!("Version: {version}\n");
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    // `new-version` output: detect "Setting version...to: N"
    if let Some((version, file_count)) = extract_new_version(raw) {
        let content = format!(
            "Version updated → {version} ({file_count} file{})\n",
            if file_count == 1 { "" } else { "s" }
        );
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    FilterOutput::passthrough(raw)
}

/// Extract version from `agvtool what-version` output.
fn extract_what_version(raw: &str) -> Option<String> {
    let lines: Vec<&str> = raw.lines().collect();

    // Simple case: just a number
    if lines.len() == 1 {
        let trimmed = lines[0].trim();
        if trimmed.chars().all(|c| c.is_ascii_digit() || c == '.') && !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    // Multi-line: "Current version of project X is:\n    N"
    for (i, line) in lines.iter().enumerate() {
        if line.contains("Current version of project") && line.contains("is:") {
            // Version is on the next line (indented)
            if let Some(next) = lines.get(i + 1) {
                let v = next.trim().trim_end_matches('.');
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }

    None
}

/// Extract new version and file count from `agvtool new-version` output.
fn extract_new_version(raw: &str) -> Option<(String, usize)> {
    let mut version: Option<String> = None;
    let mut file_count = 0usize;

    for line in raw.lines() {
        let trimmed = line.trim();

        // "Setting version of project X to:\n    N."
        if trimmed.starts_with("Setting version") && trimmed.contains("to:") {
            // Version might be on this line after "to:" or next line
            if let Some(after) = trimmed.split("to:").nth(1) {
                let v = after.trim().trim_end_matches('.');
                if !v.is_empty() {
                    version = Some(v.to_string());
                }
            }
            continue;
        }

        // "    N." — version number on its own line (after "to:")
        if version.is_none() {
            let v = trimmed.trim_end_matches('.');
            if v.chars().all(|c| c.is_ascii_digit() || c == '.') && !v.is_empty() {
                version = Some(v.to_string());
                continue;
            }
        }

        // Count Info.plist updates
        if trimmed.starts_with("Updated CFBundleVersion in") {
            file_count += 1;
        }
    }

    version.map(|v| (v, file_count))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_WHAT_VERSION: &str = "\
Current version of project MyApp is: 
    47
";

    const SAMPLE_WHAT_VERSION_SIMPLE: &str = "2\n";

    const SAMPLE_NEW_VERSION: &str = "\
Setting version of project MyApp to: 
    48.

Also setting CFBundleVersion key (assuming it exists)
Updated CFBundleVersion in \"MyApp/Info.plist\" to 48
Updated CFBundleVersion in \"MyAppTests/Info.plist\" to 48
Updated CFBundleVersion in \"MyAppUITests/Info.plist\" to 48
";

    #[test]
    fn compact_what_version_multi_line() {
        let out = filter(SAMPLE_WHAT_VERSION, Verbosity::Compact);
        assert!(out.content.contains("Version: 47"));
    }

    #[test]
    fn compact_what_version_simple() {
        let out = filter(SAMPLE_WHAT_VERSION_SIMPLE, Verbosity::Compact);
        assert!(out.content.contains("Version: 2"));
    }

    #[test]
    fn compact_new_version_shows_updated() {
        let out = filter(SAMPLE_NEW_VERSION, Verbosity::Compact);
        assert!(out.content.contains("Version updated"));
        assert!(out.content.contains("48"));
    }

    #[test]
    fn compact_new_version_counts_files() {
        let out = filter(SAMPLE_NEW_VERSION, Verbosity::Compact);
        assert!(out.content.contains("3 files"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_NEW_VERSION, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_NEW_VERSION);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_NEW_VERSION, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}
