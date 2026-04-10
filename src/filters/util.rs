//! Shared utilities used across multiple filter modules.

/// Shorten a file path to its last `keep` components for readability.
///
/// ```text
/// short_path("/Users/dev/project/Sources/App/Main.swift", 3)
/// // → "Sources/App/Main.swift"
/// ```
pub fn short_path(path: &str, keep: usize) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= keep {
        return path.to_string();
    }
    parts[parts.len() - keep..].join("/")
}

/// Return `""` for 1, `"s"` otherwise — for simple English plurals.
pub fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

/// Split a compiler diagnostic line at a severity marker (e.g. `": error:"`).
///
/// Returns `Some((location, message))` where `location` is everything before
/// the marker and `message` is everything after, trimmed. Returns `None` if
/// the marker is not found.
#[allow(dead_code)] // Used in Phase 1 structured type refactor
pub fn split_at_marker<'a>(line: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let idx = line.find(marker)?;
    let location = &line[..idx];
    let message = line[idx + marker.len()..].trim();
    Some((location, message))
}

/// Strip `:LINE:COL` suffix from a compiler diagnostic path.
///
/// ```text
/// strip_line_col("/path/to/File.swift:10:5") → "/path/to/File.swift"
/// strip_line_col("/path/to/File.swift")      → "/path/to/File.swift"
/// ```
#[allow(dead_code)] // Used in Phase 1 structured type refactor
pub fn strip_line_col(path: &str) -> &str {
    // Walk backwards: skip up to two `:DIGITS` suffixes
    let mut end = path.len();
    for _ in 0..2 {
        if let Some(colon) = path[..end].rfind(':') {
            if path[colon + 1..end].chars().all(|c| c.is_ascii_digit()) {
                end = colon;
            } else {
                break;
            }
        } else {
            break;
        }
    }
    &path[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_path_keeps_last_n_components() {
        assert_eq!(
            short_path("/Users/dev/project/Sources/App/Main.swift", 3),
            "Sources/App/Main.swift"
        );
    }

    #[test]
    fn short_path_short_input_unchanged() {
        assert_eq!(short_path("src/main.rs", 3), "src/main.rs");
    }

    #[test]
    fn short_path_exact_count_unchanged() {
        assert_eq!(short_path("/a/b/c", 3), "/a/b/c");
    }

    #[test]
    fn plural_one_is_empty() {
        assert_eq!(plural(1), "");
    }

    #[test]
    fn plural_many_is_s() {
        assert_eq!(plural(0), "s");
        assert_eq!(plural(2), "s");
        assert_eq!(plural(100), "s");
    }

    #[test]
    fn split_at_marker_finds_error() {
        let line = "/path/File.swift:10:5: error: use of unresolved identifier";
        let (loc, msg) = split_at_marker(line, ": error:").unwrap();
        assert_eq!(loc, "/path/File.swift:10:5");
        assert_eq!(msg, "use of unresolved identifier");
    }

    #[test]
    fn split_at_marker_returns_none_on_miss() {
        assert!(split_at_marker("no marker here", ": error:").is_none());
    }

    #[test]
    fn strip_line_col_removes_line_and_col() {
        assert_eq!(strip_line_col("/path/File.swift:10:5"), "/path/File.swift");
    }

    #[test]
    fn strip_line_col_removes_line_only() {
        assert_eq!(strip_line_col("/path/File.swift:10"), "/path/File.swift");
    }

    #[test]
    fn strip_line_col_no_suffix_unchanged() {
        assert_eq!(strip_line_col("/path/File.swift"), "/path/File.swift");
    }
}
