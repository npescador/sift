use std::collections::BTreeMap;

use crate::filters::{FilterOutput, Verbosity};

/// Maximum matches shown per file in Compact mode.
const COMPACT_MAX_PER_FILE: usize = 3;
/// Maximum total matches shown in Compact mode before truncation.
const COMPACT_MAX_TOTAL: usize = 30;

/// Filter `grep` / `rg` output — group by file, deduplicate, cap results.
///
/// Compact: up to 3 matches per file, 30 total, with truncation notice.
/// Verbose: all matches grouped by file with counts.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // Group matches by file. Supports "file:line:match" and "file:match" formats.
    let mut by_file: BTreeMap<&str, Vec<&str>> = BTreeMap::new();

    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let (file, content) = split_grep_line(line);
        by_file.entry(file).or_default().push(content);
    }

    if by_file.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let max_per_file = match verbosity {
        Verbosity::Compact => COMPACT_MAX_PER_FILE,
        _ => usize::MAX,
    };
    let max_total = match verbosity {
        Verbosity::Compact => COMPACT_MAX_TOTAL,
        _ => usize::MAX,
    };

    let mut out = String::new();
    let mut total_shown = 0;
    let mut total_matches = 0;
    let mut truncated = false;

    for (file, matches) in &by_file {
        total_matches += matches.len();

        if total_shown >= max_total {
            truncated = true;
            continue;
        }

        let show = matches.len().min(max_per_file);
        let remaining = matches.len().saturating_sub(max_per_file);

        out.push_str(&format!("\x1b[1m{file}\x1b[0m ({} match{})\n",
            matches.len(),
            if matches.len() == 1 { "" } else { "es" }
        ));

        for m in &matches[..show] {
            out.push_str(&format!("  {m}\n"));
            total_shown += 1;
            if total_shown >= max_total {
                truncated = true;
                break;
            }
        }

        if remaining > 0 && !truncated {
            out.push_str(&format!("  … {remaining} more match{}\n",
                if remaining == 1 { "" } else { "es" }
            ));
        }
    }

    let file_count = by_file.len();
    out.push_str(&format!(
        "\n{total_matches} match{} across {file_count} file{}\n",
        if total_matches == 1 { "" } else { "es" },
        if file_count == 1 { "" } else { "s" },
    ));

    if truncated {
        out.push_str("(output capped — use -vv for full results or --raw for raw output)\n");
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

/// Split a grep/rg output line into (file, content).
///
/// Handles formats:
/// - `file:line_number:content`  (rg default)
/// - `file:content`              (grep default)
/// - `content`                   (no file prefix, e.g. piped input)
fn split_grep_line(line: &str) -> (&str, &str) {
    // rg outputs "file:linenum:content" — find first colon that looks like a path
    let mut colon_count = 0;
    let mut first_colon = None;

    for (i, c) in line.char_indices() {
        if c == ':' {
            colon_count += 1;
            if first_colon.is_none() {
                first_colon = Some(i);
            }
            // If second colon and the part between looks like a line number, use first colon
            if colon_count == 2 {
                let between = &line[first_colon.unwrap() + 1..i];
                if between.chars().all(|c| c.is_ascii_digit()) {
                    return (&line[..first_colon.unwrap()], &line[i + 1..]);
                }
                break;
            }
        }
    }

    if let Some(pos) = first_colon {
        (&line[..pos], &line[pos + 1..])
    } else {
        ("(stdin)", line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RG: &str = "\
src/main.rs:42:    let x = executor::execute(program, rest);
src/main.rs:55:    executor::execute(cmd, args)
src/cli.rs:10:pub fn execute() {}
src/executor.rs:1:use std::process::Command;
src/executor.rs:31:pub fn execute(program: &str, args: &[String]) -> Result<ExecutorOutput, SiftError> {
";

    #[test]
    fn compact_groups_by_file() {
        let out = filter(SAMPLE_RG, Verbosity::Compact);
        assert!(out.content.contains("src/main.rs"));
        assert!(out.content.contains("src/cli.rs"));
        assert!(out.content.contains("src/executor.rs"));
        assert!(out.content.contains("5 matches across 3 files"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_RG, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_RG);
    }

    #[test]
    fn split_rg_line_with_line_number() {
        let (file, content) = split_grep_line("src/main.rs:42:let x = 1;");
        assert_eq!(file, "src/main.rs");
        assert_eq!(content, "let x = 1;");
    }

    #[test]
    fn split_grep_line_without_line_number() {
        let (file, content) = split_grep_line("src/main.rs:let x = 1;");
        assert_eq!(file, "src/main.rs");
        assert_eq!(content, "let x = 1;");
    }
}

