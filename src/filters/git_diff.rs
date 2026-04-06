use crate::filters::{FilterOutput, Verbosity};

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

    let mut out = String::new();
    let mut current_file = "";
    let mut additions: i32 = 0;
    let mut deletions: i32 = 0;
    let mut file_count = 0;
    let mut total_additions: i32 = 0;
    let mut total_deletions: i32 = 0;

    for line in raw.lines() {
        if line.starts_with("diff --git ") {
            // Flush previous file
            if !current_file.is_empty() {
                flush_file(&mut out, current_file, additions, deletions, verbosity);
                total_additions += additions;
                total_deletions += deletions;
            }
            current_file = extract_diff_filename(line);
            additions = 0;
            deletions = 0;
            file_count += 1;
            continue;
        }

        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        } else if line.starts_with("@@") && verbosity == Verbosity::Verbose {
            // Include hunk headers in verbose mode
            out.push_str(&format!("  {line}\n"));
        }
    }

    // Flush last file
    if !current_file.is_empty() {
        flush_file(&mut out, current_file, additions, deletions, verbosity);
        total_additions += additions;
        total_deletions += deletions;
    }

    if file_count == 0 {
        return FilterOutput::passthrough(raw);
    }

    // Summary line
    out.push_str(&format!(
        "\n{file_count} file{} changed  \
         \x1b[32m+{total_additions}\x1b[0m \x1b[31m-{total_deletions}\x1b[0m\n",
        if file_count == 1 { "" } else { "s" }
    ));

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

fn flush_file(out: &mut String, file: &str, additions: i32, deletions: i32, _verbosity: Verbosity) {
    out.push_str(&format!(
        "  {file:<50}  \x1b[32m+{additions}\x1b[0m \x1b[31m-{deletions}\x1b[0m\n"
    ));
}

fn extract_diff_filename(line: &str) -> &str {
    // "diff --git a/src/main.rs b/src/main.rs" → "src/main.rs"
    line.split(' ')
        .last()
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
}
