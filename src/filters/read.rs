use crate::filters::types::ReadResult;
use crate::filters::{FilterOutput, Verbosity};

/// Default max lines shown in Compact mode.
const COMPACT_MAX_LINES: usize = 100;

pub fn parse(raw: &str) -> ReadResult {
    let is_binary = is_likely_binary(raw);
    let total_lines = raw.lines().count();
    ReadResult {
        total_lines,
        shown_lines: total_lines,
        is_binary,
    }
}

/// Filter `cat` / file read output — safe truncation with line range support.
///
/// Compact: truncates to COMPACT_MAX_LINES with a notice.
/// Verbose: truncates to 2× limit.
/// VeryVerbose+: full content.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);

    // Detect binary content early — don't attempt to display it
    if result.is_binary {
        let content = "(binary file — use --raw to see raw bytes)\n".to_string();
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
            structured: serde_json::to_value(&result).ok(),
        };
    }

    let max_lines = match verbosity {
        Verbosity::Compact => COMPACT_MAX_LINES,
        Verbosity::Verbose => COMPACT_MAX_LINES * 2,
        _ => usize::MAX,
    };

    let total_lines = result.total_lines;

    if total_lines <= max_lines {
        return FilterOutput::passthrough(raw);
    }

    let lines: Vec<&str> = raw.lines().collect();
    let shown: Vec<&str> = lines[..max_lines].to_vec();
    let remaining = total_lines - max_lines;

    let result = ReadResult {
        total_lines,
        shown_lines: max_lines,
        is_binary: false,
    };

    let mut out = shown.join("\n");
    out.push('\n');
    out.push_str(&format!(
        "\n… {remaining} more line{} (use -vv or --raw to see all {total_lines} lines)\n",
        if remaining == 1 { "" } else { "s" }
    ));

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

/// Heuristic binary detection: look for null bytes in the first 8KB.
fn is_likely_binary(content: &str) -> bool {
    content.bytes().take(8192).any(|b| b == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_file_returns_passthrough() {
        let content = "line1\nline2\nline3\n";
        let out = filter(content, Verbosity::Compact);
        assert_eq!(out.content, content);
    }

    #[test]
    fn long_file_truncated_with_notice() {
        let content: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let out = filter(&content, Verbosity::Compact);
        assert!(out.content.contains("more lines"));
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let content: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let out = filter(&content, Verbosity::VeryVerbose);
        assert_eq!(out.content, content);
    }

    #[test]
    fn binary_file_shows_notice() {
        let content = "hello\0world";
        let out = filter(content, Verbosity::Compact);
        assert!(out.content.contains("binary file"));
    }

    #[test]
    fn parse_long_file_returns_structured_data() {
        let content: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let result = parse(&content);
        assert_eq!(result.total_lines, 200);
        assert!(!result.is_binary);
    }

    #[test]
    fn structured_is_some_on_truncated_filter() {
        let content: String = (0..200).map(|i| format!("line {i}\n")).collect();
        let out = filter(&content, Verbosity::Compact);
        assert!(out.structured.is_some());
    }

    #[test]
    fn structured_is_some_on_binary_filter() {
        let out = filter("hello\0world", Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
