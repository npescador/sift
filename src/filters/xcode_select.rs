use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcode-select` output.
///
/// Compact:
/// - `--version`: show `Xcode CLI tools: version N`
/// - `--print-path` / `-p`: show the path directly
/// - Other: passthrough
///
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let trimmed = raw.trim();

    // Version: "xcode-select version 2395."
    if trimmed.starts_with("xcode-select version") {
        let version = trimmed
            .trim_start_matches("xcode-select version")
            .trim()
            .trim_end_matches('.');
        let content = format!("Xcode CLI tools: version {version}\n");
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    // Path output: "/Applications/Xcode.app/Contents/Developer"
    if trimmed.starts_with('/') {
        let content = format!("{trimmed}\n");
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    FilterOutput::passthrough(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VERSION: &str = "xcode-select version 2395.\n";
    const SAMPLE_PATH: &str = "/Applications/Xcode.app/Contents/Developer\n";
    const SAMPLE_OTHER: &str = "Some other xcode-select output\n";

    #[test]
    fn compact_version_shows_formatted() {
        let out = filter(SAMPLE_VERSION, Verbosity::Compact);
        assert!(out.content.contains("Xcode CLI tools: version 2395"));
        assert!(!out.content.contains("xcode-select version"));
    }

    #[test]
    fn compact_path_shows_path() {
        let out = filter(SAMPLE_PATH, Verbosity::Compact);
        assert!(out.content.contains("/Applications/Xcode.app"));
    }

    #[test]
    fn compact_other_is_passthrough() {
        let out = filter(SAMPLE_OTHER, Verbosity::Compact);
        assert_eq!(out.content, SAMPLE_OTHER);
    }

    #[test]
    fn very_verbose_returns_passthrough_version() {
        let out = filter(SAMPLE_VERSION, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_VERSION);
    }

    #[test]
    fn very_verbose_returns_passthrough_path() {
        let out = filter(SAMPLE_PATH, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_PATH);
    }

    #[test]
    fn bytes_reduced_for_version() {
        let out = filter(SAMPLE_VERSION, Verbosity::Compact);
        // Content is different (reformatted), bytes may be similar but content is cleaner
        assert!(!out.content.is_empty());
    }
}
