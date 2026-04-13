use std::collections::BTreeMap;

use crate::filters::types::{FileMatches, GrepResult};
use crate::filters::{FilterOutput, Verbosity};

const COMPACT_MAX_PER_FILE: usize = 3;
const COMPACT_MAX_TOTAL: usize = 30;

pub fn parse(raw: &str) -> GrepResult {
    let mut by_file: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for line in raw.lines() {
        if line.is_empty() {
            continue;
        }
        let (file, content) = split_grep_line(line);
        by_file
            .entry(file.to_string())
            .or_default()
            .push(content.to_string());
    }

    let total_matches: usize = by_file.values().map(|v| v.len()).sum();
    let file_count = by_file.len();
    let files: Vec<FileMatches> = by_file
        .into_iter()
        .map(|(file, matches)| {
            let count = matches.len();
            FileMatches {
                file,
                matches,
                count,
            }
        })
        .collect();

    GrepResult {
        files,
        total_matches,
        file_count,
    }
}

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

    let result = parse(raw);

    if result.file_count == 0 {
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
    let mut truncated = false;

    for file_matches in &result.files {
        if total_shown >= max_total {
            truncated = true;
            continue;
        }

        let show = file_matches.count.min(max_per_file);
        let remaining = file_matches.count.saturating_sub(max_per_file);

        out.push_str(&format!(
            "\x1b[1m{}\x1b[0m ({} match{})\n",
            file_matches.file,
            file_matches.count,
            if file_matches.count == 1 { "" } else { "es" }
        ));

        for m in &file_matches.matches[..show] {
            out.push_str(&format!("  {m}\n"));
            total_shown += 1;
            if total_shown >= max_total {
                truncated = true;
                break;
            }
        }

        if remaining > 0 && !truncated {
            out.push_str(&format!(
                "  … {remaining} more match{}\n",
                if remaining == 1 { "" } else { "es" }
            ));
        }
    }

    out.push_str(&format!(
        "\n{} match{} across {} file{}\n",
        result.total_matches,
        if result.total_matches == 1 { "" } else { "es" },
        result.file_count,
        if result.file_count == 1 { "" } else { "s" },
    ));

    if truncated {
        out.push_str("(output capped — use -vv for full results or --raw for raw output)\n");
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

fn split_grep_line(line: &str) -> (&str, &str) {
    let mut colon_count = 0;
    let mut first_colon = None;

    for (i, c) in line.char_indices() {
        if c == ':' {
            colon_count += 1;
            if first_colon.is_none() {
                first_colon = Some(i);
            }
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

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(SAMPLE_RG);
        assert_eq!(result.file_count, 3);
        assert_eq!(result.total_matches, 5);
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE_RG, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
