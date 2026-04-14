use crate::filters::types::LsResult;
use crate::filters::{FilterOutput, Verbosity};

const RELEVANT_EXTENSIONS: &[&str] = &[
    "swift",
    "m",
    "mm",
    "h",
    "c",
    "cpp",
    "xcodeproj",
    "xcworkspace",
    "xcconfig",
    "xcscheme",
    "xctestplan",
    "storyboard",
    "xib",
    "nib",
    "strings",
    "xcstrings",
    "stringsdict",
    "entitlements",
    "plist",
    "json",
    "yaml",
    "yml",
    "toml",
    "md",
    "txt",
    "rb",
    "gemspec",
];

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

const EXCLUDED_SEGMENTS: &[&str] = &[
    ".build",
    "DerivedData",
    "__MACOSX",
    "node_modules",
    ".git",
    "Pods",
    "xcuserdata",
    "xcshareddata",
    ".swp",
];

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

const EXCLUDED_FILENAMES: &[&str] = &[".DS_Store", ".localized", "Thumbs.db"];

pub fn parse_ls(raw: &str) -> LsResult {
    let mut entries: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("total ") {
            continue;
        }
        if is_long_format_line(trimmed) {
            if let Some(name) = extract_long_format_name(trimmed) {
                if name != "." && name != ".." {
                    let is_dir = trimmed.starts_with('d');
                    if is_dir || is_relevant(name) {
                        entries.push(name.to_string());
                    }
                }
            }
        } else {
            if trimmed != "." && trimmed != ".." && is_relevant(trimmed) {
                entries.push(trimmed.to_string());
            }
        }
    }

    let total_shown = entries.len();
    LsResult {
        entries,
        total_shown,
    }
}

/// Filter `ls` or `ls -la` output to Xcode-relevant entries.
pub fn filter_ls(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse_ls(raw);

    let mut kept = Vec::new();
    let mut total_line: Option<&str> = None;

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("total ") {
            total_line = Some(line);
            continue;
        }
        if is_long_format_line(trimmed) {
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
        structured: serde_json::to_value(&result).ok(),
    }
}

/// Filter `find` output to Xcode-relevant paths.
///
/// Compact: flat list of kept paths + exclusion summary line.
/// Verbose: paths grouped by parent directory + exclusion summary.
/// VeryVerbose+: raw passthrough.
pub fn filter_find(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut excluded_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    let mut total_lines = 0usize;
    let mut kept: Vec<String> = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        total_lines += 1;

        if let Some(seg) = first_excluded_segment(trimmed) {
            *excluded_counts.entry(seg).or_insert(0) += 1;
            continue;
        }

        let name = path_filename(trimmed);
        if is_relevant(name) || is_directory_entry(trimmed) {
            kept.push(trimmed.to_string());
        } else {
            *excluded_counts.entry("(other)").or_insert(0) += 1;
        }
    }

    if kept.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let result = LsResult {
        total_shown: kept.len(),
        entries: kept.clone(),
    };

    let out = match verbosity {
        Verbosity::Verbose => format_grouped(&kept, &excluded_counts, total_lines),
        _ => format_flat(&kept, &excluded_counts, total_lines),
    };

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

/// Flat list output (Compact) — one path per line + trailing exclusion summary.
fn format_flat(
    entries: &[String],
    excluded: &std::collections::HashMap<&str, usize>,
    total: usize,
) -> String {
    let mut out = String::new();
    for e in entries {
        out.push_str(e);
        out.push('\n');
    }
    push_exclusion_summary(&mut out, excluded, total, entries.len());
    out
}

/// Grouped output (Verbose) — entries grouped under their parent directory header.
fn format_grouped(
    entries: &[String],
    excluded: &std::collections::HashMap<&str, usize>,
    total: usize,
) -> String {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for entry in entries {
        let parent = path_parent(entry);
        groups.entry(parent.to_string()).or_default().push(entry);
    }

    let mut out = String::new();
    for (dir, files) in &groups {
        out.push_str(dir);
        out.push_str("/\n");
        for f in files {
            out.push_str("  ");
            out.push_str(path_filename(f));
            out.push('\n');
        }
    }

    push_exclusion_summary(&mut out, excluded, total, entries.len());
    out
}

fn push_exclusion_summary(
    out: &mut String,
    excluded: &std::collections::HashMap<&str, usize>,
    total: usize,
    kept: usize,
) {
    let n_excluded = total.saturating_sub(kept);
    if n_excluded == 0 || excluded.is_empty() {
        return;
    }
    let mut pairs: Vec<(&&str, &usize)> = excluded.iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(a.1));
    let detail: Vec<String> = pairs
        .iter()
        .filter(|(k, _)| **k != "(other)")
        .map(|(k, v)| format!("{} ×{}", k, v))
        .collect();
    if detail.is_empty() {
        out.push_str(&format!("({n_excluded} paths excluded)\n"));
    } else {
        out.push_str(&format!(
            "({n_excluded} paths excluded: {})\n",
            detail.join(", ")
        ));
    }
}

/// Returns the first excluded segment name if the path should be excluded.
fn first_excluded_segment(path: &str) -> Option<&'static str> {
    for seg in EXCLUDED_SEGMENTS {
        if path.split('/').any(|part| part == *seg)
            || path.contains(&format!("/{seg}/"))
            || path.starts_with(&format!("{seg}/"))
        {
            return Some(seg);
        }
    }
    None
}

/// Return the parent directory portion of a path.
fn path_parent(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if let Some(pos) = trimmed.rfind('/') {
        let parent = &trimmed[..pos];
        if parent.is_empty() {
            "."
        } else {
            parent
        }
    } else {
        "."
    }
}

fn is_long_format_line(line: &str) -> bool {
    matches!(
        line.chars().next(),
        Some('d' | '-' | 'l' | 'c' | 'b' | 's' | 'p')
    ) && line.len() > 10
        && line.chars().nth(1).is_some_and(|c| matches!(c, 'r' | '-'))
}

fn extract_long_format_name(line: &str) -> Option<&str> {
    let mut fields_seen = 0;
    let mut pos = 0;
    let bytes = line.as_bytes();
    let len = bytes.len();

    while pos < len && fields_seen < 8 {
        while pos < len && bytes[pos] == b' ' {
            pos += 1;
        }
        while pos < len && bytes[pos] != b' ' {
            pos += 1;
        }
        fields_seen += 1;
    }

    while pos < len && bytes[pos] == b' ' {
        pos += 1;
    }

    if pos >= len || fields_seen < 8 {
        return None;
    }

    let rest = &line[pos..];
    Some(rest.split(" -> ").next().unwrap_or(rest).trim())
}

fn path_filename(path: &str) -> &str {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
}

fn is_directory_entry(path: &str) -> bool {
    let name = path_filename(path);
    !name.contains('.') && !name.is_empty()
}

fn is_relevant(name: &str) -> bool {
    if RELEVANT_FILENAMES.contains(&name) {
        return true;
    }
    if EXCLUDED_FILENAMES.contains(&name) {
        return false;
    }
    let ext = name.rsplit('.').next().unwrap_or("");
    if EXCLUDED_EXTENSIONS.contains(&ext) {
        return false;
    }
    RELEVANT_EXTENSIONS.contains(&ext)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // Paths under .build and DerivedData should not appear as full paths
        assert!(!out.content.contains(".build/debug/MyApp"));
        assert!(!out.content.contains("DerivedData/Build"));
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
        assert!(first_excluded_segment("./.build/debug/sift").is_some());
        assert!(first_excluded_segment("./DerivedData/Build/foo.o").is_some());
        assert!(first_excluded_segment("./Sources/App/ContentView.swift").is_none());
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

    #[test]
    fn parse_ls_returns_structured_data() {
        let result = parse_ls(LS_LONG);
        assert!(result.entries.contains(&"ContentView.swift".to_string()));
        assert!(!result.entries.contains(&"Foo.o".to_string()));
    }

    #[test]
    fn structured_is_some_on_filter_ls() {
        let out = filter_ls(LS_LONG, Verbosity::Compact);
        assert!(out.structured.is_some());
    }

    #[test]
    fn structured_is_some_on_filter_find() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Compact);
        assert!(out.structured.is_some());
    }

    const FIND_WITH_PODS: &str = "\
./Package.swift
./Sources/MyApp/ContentView.swift
./Pods/Firebase/FirebaseCore.h
./Pods/Firebase/FirebaseCore.m
./Pods/Alamofire/Alamofire.swift
./DerivedData/Build/foo.o
./Tests/MyAppTests/Tests.swift
";

    #[test]
    fn find_excludes_pods_directory() {
        let out = filter_find(FIND_WITH_PODS, Verbosity::Compact);
        assert!(!out.content.contains("Firebase"));
        assert!(!out.content.contains("Alamofire"));
    }

    #[test]
    fn find_compact_shows_exclusion_summary() {
        let out = filter_find(FIND_WITH_PODS, Verbosity::Compact);
        assert!(out.content.contains("paths excluded"));
        assert!(out.content.contains("Pods"));
    }

    #[test]
    fn find_verbose_groups_by_directory() {
        let out = filter_find(FIND_OUTPUT, Verbosity::Verbose);
        // Grouped format shows directory header with trailing /
        assert!(out.content.contains("./Sources/MyApp/"));
        // Filenames shown under directory (indented)
        assert!(out.content.contains("  ContentView.swift"));
    }

    #[test]
    fn find_verbose_shows_exclusion_summary() {
        let out = filter_find(FIND_WITH_PODS, Verbosity::Verbose);
        assert!(out.content.contains("paths excluded"));
    }

    #[test]
    fn path_parent_extracts_correctly() {
        assert_eq!(
            path_parent("./Sources/MyApp/ContentView.swift"),
            "./Sources/MyApp"
        );
        assert_eq!(path_parent("./Package.swift"), ".");
        assert_eq!(path_parent("Package.swift"), ".");
    }
}
