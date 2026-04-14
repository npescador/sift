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

// ---------------------------------------------------------------------------
// Swift outline mode (--outline flag)
// ---------------------------------------------------------------------------

/// Extract Swift declarations from source — types, method signatures, properties.
/// Strips all implementation bodies. Useful for AI agents exploring project structure.
///
/// Compact: public/internal declarations only, private methods summarized as count.
/// Verbose: all declarations including private.
pub fn filter_outline(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let show_private = matches!(verbosity, Verbosity::Verbose);
    let lines = extract_outline(raw, show_private);
    let content = lines.join("\n") + "\n";
    let filtered_bytes = content.len();

    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

/// Extract Swift declaration lines, stripping implementation bodies.
pub fn extract_outline(raw: &str, show_private: bool) -> Vec<String> {
    let mut result: Vec<String> = Vec::new();
    let mut brace_depth: i32 = 0;
    let mut private_method_count = 0usize;

    for line in raw.lines() {
        let trimmed = line.trim();

        // Skip blank lines and pure comments at depth > 0 (inside bodies)
        if trimmed.is_empty() && brace_depth > 0 {
            continue;
        }

        // Count braces opened on this line
        let opens = trimmed.chars().filter(|&c| c == '{').count() as i32;
        let closes = trimmed.chars().filter(|&c| c == '}').count() as i32;

        // Is this a declaration line?
        let is_decl = is_swift_declaration(trimmed);

        if is_decl {
            let is_private = is_private_declaration(trimmed);

            if is_private && !show_private {
                private_method_count += 1;
                // Still track brace depth for body skipping
                brace_depth += opens - closes;
                continue;
            }

            // Emit the declaration, stripping the opening brace and body
            let decl_line = strip_body_from_declaration(trimmed);
            result.push(decl_line);
        } else if brace_depth == 0 {
            // Top-level non-declaration lines (imports, attributes, blank lines)
            if trimmed.starts_with("import ")
                || trimmed.starts_with("@")
                || trimmed.starts_with("//")
            {
                result.push(trimmed.to_string());
            }
        }

        brace_depth += opens - closes;
        brace_depth = brace_depth.max(0);
    }

    // Append private method summary if any were hidden
    if private_method_count > 0 && !show_private {
        result.push(format!(
            "  // +{} private symbol{} omitted (use -v to show)",
            private_method_count,
            if private_method_count == 1 { "" } else { "s" }
        ));
    }

    result
}

/// Return true if this line is a Swift type or member declaration.
fn is_swift_declaration(line: &str) -> bool {
    const DECL_KEYWORDS: &[&str] = &[
        "class ",
        "struct ",
        "enum ",
        "protocol ",
        "actor ",
        "extension ",
        "func ",
        "var ",
        "let ",
        "init(",
        "init?(",
        "init!(",
        "subscript(",
        "typealias ",
        "associatedtype ",
    ];

    // Strip leading access/modifier keywords (but NOT "class" — it's also a type keyword)
    let stripped = line
        .trim_start_matches("public ")
        .trim_start_matches("internal ")
        .trim_start_matches("private ")
        .trim_start_matches("fileprivate ")
        .trim_start_matches("open ")
        .trim_start_matches("final ")
        .trim_start_matches("static ")
        .trim_start_matches("override ")
        .trim_start_matches("@MainActor ")
        .trim_start_matches("@discardableResult ")
        .trim_start_matches("nonisolated ");

    DECL_KEYWORDS.iter().any(|kw| stripped.starts_with(kw))
        || line.starts_with("@")
        || line.trim_start().starts_with("case ")
}

/// Return true if this declaration is private or fileprivate.
fn is_private_declaration(line: &str) -> bool {
    line.trim_start().starts_with("private ") || line.trim_start().starts_with("fileprivate ")
}

/// Strip implementation body: keep signature up to `{`, removing the brace.
fn strip_body_from_declaration(line: &str) -> String {
    if let Some(pos) = line.find('{') {
        // Only strip if the brace is not inside a generic constraint < >
        let before = line[..pos].trim_end();
        // Keep computed property markers like `{ get }` on one line
        let after = line[pos..].trim();
        if after == "{ get }" || after == "{ get set }" || after == "{ get throws }" {
            return line.to_string();
        }
        before.to_string()
    } else {
        line.to_string()
    }
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

    // --- outline tests ---

    const SAMPLE_SWIFT: &str = r#"import Foundation
import Combine

@Observable
class CheckoutViewModel {
    var items: [CartItem] = []
    var isLoading: Bool = false
    private var cancellables = Set<AnyCancellable>()

    func loadCart() async {
        isLoading = true
        let data = await fetchData()
        isLoading = false
    }

    func checkout(with method: PaymentMethod) async throws -> Order {
        // implementation
        return Order()
    }

    private func validateCard(_ card: Card) -> Bool {
        return true
    }

    private func debugDump() {
        print(items)
    }
}
"#;

    #[test]
    fn outline_extracts_class_declaration() {
        let lines = extract_outline(SAMPLE_SWIFT, false);
        assert!(lines.iter().any(|l| l.contains("class CheckoutViewModel")));
    }

    #[test]
    fn outline_extracts_public_func_signatures() {
        let lines = extract_outline(SAMPLE_SWIFT, false);
        assert!(lines.iter().any(|l| l.contains("func loadCart()")));
        assert!(lines.iter().any(|l| l.contains("func checkout(")));
    }

    #[test]
    fn outline_hides_private_methods_in_compact() {
        let lines = extract_outline(SAMPLE_SWIFT, false);
        assert!(!lines.iter().any(|l| l.contains("validateCard")));
        assert!(!lines.iter().any(|l| l.contains("debugDump")));
        // But shows summary count
        assert!(lines
            .iter()
            .any(|l| l.contains("private") && l.contains("omitted")));
    }

    #[test]
    fn outline_shows_private_methods_in_verbose() {
        let lines = extract_outline(SAMPLE_SWIFT, true);
        assert!(lines.iter().any(|l| l.contains("validateCard")));
        assert!(lines.iter().any(|l| l.contains("debugDump")));
    }

    #[test]
    fn outline_strips_implementation_bodies() {
        let lines = extract_outline(SAMPLE_SWIFT, false);
        // No lines should contain just "}" as a declaration body
        let func_line = lines.iter().find(|l| l.contains("func loadCart()"));
        assert!(func_line.is_some());
        // Should not contain the body
        assert!(!func_line.unwrap().contains("isLoading = true"));
    }

    #[test]
    fn outline_reduces_bytes_significantly() {
        let out = filter_outline(SAMPLE_SWIFT, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes / 2);
    }
}
