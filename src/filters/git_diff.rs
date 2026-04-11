use crate::filters::types::{DiffFile, GitDiffResult};
use crate::filters::{FilterOutput, Verbosity};

pub fn parse(raw: &str) -> GitDiffResult {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut current_file = "";
    let mut additions: i32 = 0;
    let mut deletions: i32 = 0;
    let mut total_additions: i32 = 0;
    let mut total_deletions: i32 = 0;

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            if !current_file.is_empty() {
                total_additions += additions;
                total_deletions += deletions;
                files.push(DiffFile {
                    path: current_file.to_string(),
                    additions,
                    deletions,
                });
            }
            current_file = extract_diff_filename(line);
            additions = 0;
            deletions = 0;
            continue;
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }

    if !current_file.is_empty() {
        total_additions += additions;
        total_deletions += deletions;
        files.push(DiffFile {
            path: current_file.to_string(),
            additions,
            deletions,
        });
    }

    let file_count = files.len();
    GitDiffResult {
        files,
        total_additions,
        total_deletions,
        file_count,
    }
}

/// Filter `git diff` output into a compact per-file summary.
///
/// Compact: shows changed files with +/- line counts only.
/// Verbose: adds hunk headers (@@) without raw diff content.
/// VeryVerbose+: full diff.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);

    if result.file_count == 0 {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();

    // Re-scan for hunk headers in verbose mode (not stored in parse result)
    let mut file_hunks: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();
    if verbosity == Verbosity::Verbose {
        let mut cur = "";
        for line in raw.lines() {
            if line.starts_with("diff --git ") {
                cur = extract_diff_filename(line);
            } else if line.starts_with("@@") {
                file_hunks.entry(cur).or_default().push(line);
            }
        }
    }

    for file in &result.files {
        out.push_str(&format!(
            "  {:<50}  \x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m\n",
            file.path, file.additions, file.deletions
        ));
        if verbosity == Verbosity::Verbose {
            if let Some(hunks) = file_hunks.get(file.path.as_str()) {
                for h in hunks {
                    out.push_str(&format!("  {h}\n"));
                }
            }
        }
    }

    out.push_str(&format!(
        "\n{} file{} changed  \
         \x1b[32m+{}\x1b[0m \x1b[31m-{}\x1b[0m\n",
        result.file_count,
        if result.file_count == 1 { "" } else { "s" },
        result.total_additions,
        result.total_deletions,
    ));

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

fn extract_diff_filename(line: &str) -> &str {
    line.split(' ')
        .next_back()
        .and_then(|s| s.strip_prefix("b/"))
        .unwrap_or(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_DIFF: &str = "\
diff --git a/src/main.rs b/src/main.rs
index abc..def 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,5 @@
 fn main() {
+    let x = 1;
+    println!(\"{}\", x);
 }
diff --git a/src/cli.rs b/src/cli.rs
index 111..222 100644
--- a/src/cli.rs
+++ b/src/cli.rs
@@ -10,2 +10,3 @@
-    old_line,
+    new_line,
";

    #[test]
    fn compact_shows_file_stats() {
        let out = filter(SAMPLE_DIFF, Verbosity::Compact);
        assert!(out.content.contains("src/main.rs"));
        assert!(out.content.contains("src/cli.rs"));
        assert!(out.content.contains("2 files changed"));
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_DIFF, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_DIFF);
    }

    #[test]
    fn extracts_filename_correctly() {
        let line = "diff --git a/src/main.rs b/src/main.rs";
        assert_eq!(extract_diff_filename(line), "src/main.rs");
    }

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(SAMPLE_DIFF);
        assert_eq!(result.file_count, 2);
        assert_eq!(result.total_additions, 3);
        assert_eq!(result.total_deletions, 1);
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE_DIFF, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
