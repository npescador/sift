use crate::filters::{FilterOutput, Verbosity};

// ── Allowlists ────────────────────────────────────────────────────────────────

/// File extensions considered relevant in an Xcode/Swift project.
const RELEVANT_EXTENSIONS: &[&str] = &[
    // Swift & ObjC source
    "swift",
    "m",
    "mm",
    "h",
    "c",
    "cpp",
    // Xcode project containers
    "xcodeproj",
    "xcworkspace",
    "xcconfig",
    "xcscheme",
    "xctestplan",
    // Interface builder (legacy)
    "storyboard",
    "xib",
    "nib",
    // Localization
    "strings",
    "xcstrings",
    "stringsdict",
    // Manifest & config
    "entitlements",
    "plist",
    // Package / dependency
    "json",
    "yaml",
    "yml",
    "toml",
    // Docs
    "md",
    "txt",
    // Ruby tooling (fastlane, CocoaPods)
    "rb",
    "gemspec",
];

/// Exact filenames that are always relevant regardless of extension.
const RELEVANT_FILENAMES: &[&str] = &[
    "Package.swift",
    "Package.resolved",
    "Podfile",
    "Podfile.lock",
    "Gemfile",
    "Gemfile.lock",
    "Fastfile",
    "Appfile",
    "Matchfile",
    "Deliverfile",
    ".swiftlint.yml",
    ".swiftformat",
    "Makefile",
    "Dockerfile",
];

// ── Denylists ─────────────────────────────────────────────────────────────────

/// Path segments that indicate build/cache/generated directories to skip.
const EXCLUDED_SEGMENTS: &[&str] = &[
    ".build",
    "DerivedData",
    "__MACOSX",
    "node_modules",
    ".git",
    "Pods/Pods", // CocoaPods build dir (not the Podfile itself)
    "xcuserdata",
    "xcshareddata",
    ".swp",
];

/// File extensions that are always noise.
const EXCLUDED_EXTENSIONS: &[&str] = &[
    "o",
    "d",
    "a",
    "la",
    "lo",
    "dylib",
    "so",
    "exe",
    "dSYM",
    "map",
    "lto_passes",
    "pyc",
    "pyo",
];

/// Exact filenames that are always noise.
const EXCLUDED_FILENAMES: &[&str] = &[".DS_Store", ".localized", "Thumbs.db"];

// ── Filter entry points ───────────────────────────────────────────────────────

/// Filter `ls` or `ls -la` output to Xcode-relevant entries.
///
/// Handles two formats:
/// - Long format (`ls -l`, `ls -la`): lines starting with permission characters.
/// - Single-column (`ls -1`, plain `ls`): one name per line.
///
/// Directories are always kept so the tree structure remains navigable.
pub fn filter_ls(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut kept = Vec::new();
    let mut total_line: Option<&str> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Preserve the `total N` header line from `ls -la`
        if trimmed.starts_with("total ") {
            total_line = Some(line);
            continue;
        }

        if is_long_format_line(trimmed) {
            // Long format: extract filename from last column (after date)
            if let Some(name) = extract_long_format_name(trimmed) {
                if name == "." || name == ".." {
                    continue;
                }
                let is_dir = trimmed.starts_with('d');
                if is_dir || is_relevant(name) {
                    kept.push(line);
                }
            }
        } else {
            // Plain / single-column: the whole line is the name
            if trimmed == "." || trimmed == ".." {
                continue;
            }
            if is_relevant(trimmed) {
                kept.push(line);
            }
        }
    }

    if kept.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();
    if let Some(total) = total_line {
        out.push_str(total);
        out.push('\n');
    }
    for line in &kept {
        out.push_str(line);
        out.push('\n');
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

/// Filter `find` output to Xcode-relevant paths.
///
/// Each line is expected to be a file path (relative or absolute).
/// Lines containing excluded directory segments are dropped.
/// Only paths with relevant extensions or filenames are kept.
pub fn filter_find(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut kept = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Drop paths that go through excluded segments
        if has_excluded_segment(trimmed) {
            continue;
        }

        // Keep directories (lines ending with `/` or with no extension)
        let name = path_filename(trimmed);
        if is_relevant(name) || is_directory_entry(trimmed) {
            kept.push(line);
        }
    }

    if kept.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let out = kept.join("\n") + "\n";
    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Return true if the line looks like `ls -l` long format.
///
/// Long format lines begin with a permission block: `drwxr-xr-x` or `-rw-r--r--`
/// or similar (first char is one of `d`, `-`, `l`, `c`, `b`, `s`, `p`).
fn is_long_format_line(line: &str) -> bool {
    matches!(
        line.chars().next(),
        Some('d' | '-' | 'l' | 'c' | 'b' | 's' | 'p')
    ) && line.len() > 10
        && line.chars().nth(1).is_some_and(|c| matches!(c, 'r' | '-'))
}

/// Extract the filename from an `ls -l` formatted line.
///
/// Format: `permissions  links  user  group  size  month  day  time  name`
/// Skips 8 whitespace-separated fields, returns everything from the 9th onward.
/// Symlinks include ` -> target` — we take only the link name.
fn extract_long_format_name(line: &str) -> Option<&str> {
    let mut fields_seen = 0;
    let mut pos = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();

    // Skip 8 fields (each field: skip spaces, then skip non-spaces)
    while pos < len && fields_seen < 8 {
        while pos < len && bytes[pos] == b' ' {
            pos += 1;
        }
        while pos < len && bytes[pos] != b' ' {
            pos += 1;
        }
        fields_seen += 1;
    }

    // Skip leading whitespace before the 9th field (the name)
    while pos < len && bytes[pos] == b' ' {
        pos += 1;
    }

    if pos >= len || fields_seen < 8 {
        return None;
    }

    let rest = &line[pos..];
    Some(rest.split(" -> ").next().unwrap_or(rest).trim())
}

/// Return the filename component of a path (last path segment).
fn path_filename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

/// Return true if the path appears to be a directory (heuristic: no extension).
fn is_directory_entry(path: &str) -> bool {
    let name = path_filename(path);
    !name.contains('.') && !name.is_empty()
}

/// Return true if any path segment matches an excluded segment.
fn has_excluded_segment(path: &str) -> bool {
    EXCLUDED_SEGMENTS.iter().any(|seg| {
        path.split('/').any(|part| part == *seg)
            || path.contains(&format!("/{seg}/"))
            || path.starts_with(&format!("{seg}/"))
    })
}

/// Return true if a filename (not a full path) is Xcode-relevant.
fn is_relevant(name: &str) -> bool {
    // Exact filename match
    if RELEVANT_FILENAMES.contains(&name) {
        return true;
    }
    // Excluded filename
    if EXCLUDED_FILENAMES.contains(&name) {
        return false;
    }
    // Check extension
    let ext = name.rsplit('.').next().unwrap_or("");
    if EXCLUDED_EXTENSIONS.contains(&ext) {
        return false;
    }
    RELEVANT_EXTENSIONS.contains(&ext)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── ls tests ──────────────────────────────────────────────────────────────

    const LS_LONG: &str = "\
total 64
drwxr-xr-x  12 user  staff   384 Apr  7 09:00 .
drwxr-xr-x   5 user  staff   160 Apr  6 10:00 ..
drwxr-xr-x   3 user  staff    96 Apr  5 08:00 .build
-rw-r--r--   1 user  staff  1234 Apr  7 09:00 Package.swift
-rw-r--r--   1 user  staff   500 Apr  5 08:00 README.md
drwxr-xr-x   4 user  staff   128 Apr  5 08:00 Sources
-rw-r--r--   1 user  staff   800 Apr  5 08:00 ContentView.swift
-rw-r--r--   1 user  staff    45 Apr  4 12:00 libFoo.a
-rw-r--r--   1 user  staff    32 Apr  4 12:00 Foo.o
-rw-r--r--   1 user  staff    12 Apr  4 12:00 .DS_Store
";

    #[test]
    fn ls_long_keeps_swift_files() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("Package.swift"));
        assert!(out.content.contains("README.md"));
    }

    #[test]
    fn ls_long_keeps_directories() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(out.content.contains("Sources"));
    }

    #[test]
    fn ls_long_drops_build_artifacts() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(!out.content.contains("libFoo.a"));
        assert!(!out.content.contains("Foo.o"));
    }

    #[test]
    fn ls_long_drops_ds_store() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(!out.content.contains(".DS_Store"));
    }

    #[test]
    fn ls_long_drops_dot_and_dotdot() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        // `. ` and `.. ` entries should not appear
        let lines: Vec<&str> = out.content.lines().collect();
        assert!(!lines
            .iter()
            .any(|l| l.ends_with(" .") || l.ends_with(" ..")));
    }

    #[test]
    fn ls_very_verbose_passes_through() {
        let out = filter_ls(LS_LONG, Verbosity::VeryVerbose);
        assert_eq!(out.content, LS_LONG);
    }

    #[test]
    fn ls_bytes_reduced() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    // ── find tests ────────────────────────────────────────────────────────────

    const FIND_OUTPUT: &str = "\
./Package.swift
./Sources/MyApp/ContentView.swift
./Sources/MyApp/ViewModel.swift
./.build/debug/MyApp
./.build/debug/MyApp.swiftmodule
./DerivedData/Build/Products/Debug/MyApp.app
./Tests/MyAppTests/AppTests.swift
./MyApp.xcodeproj/project.pbxproj
./README.md
./some-binary.o
";

    #[test]
    fn find_keeps_swift_and_xcodeproj() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Compact);
        assert!(out.content.contains("ContentView.swift"));
        assert!(out.content.contains("ViewModel.swift"));
        assert!(out.content.contains("AppTests.swift"));
        assert!(out.content.contains("Package.swift"));
    }

    #[test]
    fn find_drops_build_paths() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Compact);
        assert!(!out.content.contains(".build/debug/MyApp"));
        assert!(!out.content.contains("DerivedData"));
    }

    #[test]
    fn find_drops_object_files() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Compact);
        assert!(!out.content.contains(".o"));
    }

    #[test]
    fn find_very_verbose_passes_through() {
        let out = filter_find(FIND_OUTPUT, Verbosity::VeryVerbose);
        assert_eq!(out.content, FIND_OUTPUT);
    }

    #[test]
    fn find_bytes_reduced() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    // ── helper unit tests ─────────────────────────────────────────────────────

    #[test]
    fn is_relevant_swift_file() {
        assert!(is_relevant("ContentView.swift"));
        assert!(is_relevant("Package.swift"));
        assert!(is_relevant("README.md"));
    }

    #[test]
    fn is_relevant_rejects_artifacts() {
        assert!(!is_relevant("foo.o"));
        assert!(!is_relevant("libFoo.a"));
        assert!(!is_relevant(".DS_Store"));
    }

    #[test]
    fn has_excluded_segment_build() {
        assert!(has_excluded_segment("./.build/debug/sift"));
        assert!(has_excluded_segment("./DerivedData/Build/foo.o"));
        assert!(!has_excluded_segment("./Sources/App/ContentView.swift"));
    }

    #[test]
    fn path_filename_extracts_correctly() {
        assert_eq!(
            path_filename("./Sources/App/ContentView.swift"),
            "ContentView.swift"
        );
        assert_eq!(path_filename("Package.swift"), "Package.swift");
        assert_eq!(path_filename("./Sources/"), "Sources");
    }
}
