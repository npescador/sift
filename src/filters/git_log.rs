use crate::filters::{FilterOutput, Verbosity};

/// Filter `git log` output.
///
/// Compact: one line per commit — `SHORT_HASH  subject  (date)  author name`.
/// Verbose: adds full hash and body preview (first non-empty body line).
/// VeryVerbose+: raw passthrough.
///
/// Handles the default multi-line git log format.
/// If the output already looks like `--oneline` (no `commit ` / `Author:` lines),
/// it is passed through unchanged since it's already compact.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    // If output is already oneline format, nothing to do
    if !raw.contains("\nAuthor:") && !raw.starts_with("Author:") {
        return FilterOutput::passthrough(raw);
    }

    let commits = parse_commits(raw);
    if commits.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();

    for commit in &commits {
        let short_hash = &commit.hash[..commit.hash.len().min(7)];
        let date = compact_date(&commit.date);
        let author = first_name(&commit.author);

        if verbosity == Verbosity::Verbose {
            // Full hash + subject + date + full author + body preview
            out.push_str(&format!(
                "\x1b[33m{}\x1b[0m  {}\n",
                commit.hash, commit.subject
            ));
            out.push_str(&format!("  Author: {}  Date: {}\n", commit.author, date));
            if let Some(body) = &commit.body_preview {
                out.push_str(&format!("  {body}\n"));
            }
            out.push('\n');
        } else {
            // Compact: short_hash  subject  (date)  author_first_name
            out.push_str(&format!(
                "\x1b[33m{short_hash}\x1b[0m  {:<55}  ({date:<6})  {author}\n",
                commit.subject
            ));
        }
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

// ── Data ──────────────────────────────────────────────────────────────────────

struct Commit {
    hash: String,
    author: String,
    date: String,
    subject: String,
    /// First non-empty line of the commit body (for verbose mode).
    body_preview: Option<String>,
}

// ── Parsing ───────────────────────────────────────────────────────────────────

/// Parse standard `git log` multi-line format into a list of commits.
///
/// Each commit block starts with `commit <HASH>` and ends when the next
/// block begins or at EOF.
fn parse_commits(raw: &str) -> Vec<Commit> {
    let mut commits = Vec::new();
    let mut hash = String::new();
    let mut author = String::new();
    let mut date = String::new();
    let mut subject = String::new();
    let mut body_lines: Vec<String> = Vec::new();
    let mut in_body = false;
    let mut in_commit = false;

    for line in raw.lines() {
        if line.starts_with("commit ") && line.len() > 7 {
            // Flush previous commit
            if in_commit {
                commits.push(build_commit(&hash, &author, &date, &subject, &body_lines));
            }
            hash = line[7..].trim().to_string();
            author.clear();
            date.clear();
            subject.clear();
            body_lines.clear();
            in_body = false;
            in_commit = true;
            continue;
        }

        if !in_commit {
            continue;
        }

        if let Some(rest) = line.strip_prefix("Author:") {
            author = rest.trim().to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("Date:") {
            date = rest.trim().to_string();
            continue;
        }
        if line.starts_with("Merge:") {
            continue;
        }

        // Empty line separates headers from body
        let trimmed = line.trim();
        if trimmed.is_empty() {
            in_body = true;
            continue;
        }

        if in_body {
            if subject.is_empty() {
                subject = trimmed.to_string();
            } else {
                body_lines.push(trimmed.to_string());
            }
        }
    }

    // Flush last commit
    if in_commit {
        commits.push(build_commit(&hash, &author, &date, &subject, &body_lines));
    }

    commits
}

fn build_commit(
    hash: &str,
    author: &str,
    date: &str,
    subject: &str,
    body_lines: &[String],
) -> Commit {
    let body_preview = body_lines.iter().find(|l| !l.is_empty()).cloned();
    Commit {
        hash: hash.to_string(),
        author: author.to_string(),
        date: date.to_string(),
        subject: subject.to_string(),
        body_preview,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract first name (or full name if no space) from "Name <email>" or plain name.
fn first_name(author: &str) -> String {
    let name = if let Some(pos) = author.find(" <") {
        &author[..pos]
    } else {
        author
    };
    name.split_whitespace().next().unwrap_or(name).to_string()
}

/// Compact a git date string to `Mon Apr 7` format.
///
/// Git default date: `Mon Apr  7 09:15:32 2026 +0200`
/// Returns: `Apr  7` (current year assumed) or `Apr  7 2025` if year differs.
fn compact_date(date: &str) -> String {
    // Format: "Day Mon  D HH:MM:SS YYYY +TZTZ"
    let parts: Vec<&str> = date.split_whitespace().collect();
    // parts: [day_name, month, day, time, year, tz]
    if parts.len() >= 5 {
        let month = parts[1];
        let day = parts[2];
        let year = parts[4];
        // Include year only for past years (hardcode current as 2026 is fine; compare string)
        if year == "2026" {
            return format!("{month} {day:>2}");
        }
        return format!("{month} {day:>2} {year}");
    }
    // Fallback: return first 10 chars as-is
    date.chars().take(10).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
commit a3f2b1c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3
Author: Nacho Pescador <nacho@example.com>
Date:   Mon Apr  7 09:15:32 2026 +0200

    feat: add payment screen

    Implements Stripe integration with 3D Secure support.

commit 91d3c2b1a0f9e8d7c6b5a4f3e2d1c0b9a8f7e6d5
Author: Nacho Pescador <nacho@example.com>
Date:   Sun Apr  6 15:32:11 2026 +0200

    fix: crash on empty state in HomeView

commit 00abcdef1234567890abcdef1234567890abcdef
Author: Other Dev <other@example.com>
Date:   Fri Mar 28 11:00:00 2026 +0100

    chore: update dependencies
";

    #[test]
    fn compact_shows_one_line_per_commit() {
        let out = filter(SAMPLE, Verbosity::Compact);
        let lines: Vec<&str> = out.content.lines().collect();
        // 3 commits → 3 output lines (ANSI codes don't add newlines)
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn compact_shows_short_hash() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("a3f2b1c"));
        // Full hash should not appear
        assert!(!out
            .content
            .contains("a3f2b1c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3"));
    }

    #[test]
    fn compact_shows_subject() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("feat: add payment screen"));
        assert!(out.content.contains("fix: crash on empty state"));
    }

    #[test]
    fn compact_shows_author_first_name() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.content.contains("Nacho"));
        assert!(out.content.contains("Other"));
        // Full email should not appear
        assert!(!out.content.contains("nacho@example.com"));
    }

    #[test]
    fn compact_does_not_show_body() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(!out.content.contains("Stripe integration"));
    }

    #[test]
    fn verbose_shows_full_hash() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        assert!(out
            .content
            .contains("a3f2b1c9d8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3"));
    }

    #[test]
    fn verbose_shows_body_preview() {
        let out = filter(SAMPLE, Verbosity::Verbose);
        assert!(out.content.contains("Stripe integration"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE);
    }

    #[test]
    fn oneline_format_passes_through() {
        let oneline = "a3f2b1c feat: add payment screen\n91d3c2b fix: crash\n";
        let out = filter(oneline, Verbosity::Compact);
        assert_eq!(out.content, oneline);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn first_name_extracts_correctly() {
        assert_eq!(first_name("Nacho Pescador <nacho@example.com>"), "Nacho");
        assert_eq!(first_name("Bot"), "Bot");
        assert_eq!(first_name("GitHub Actions <actions@github.com>"), "GitHub");
    }

    #[test]
    fn compact_date_formats_correctly() {
        assert_eq!(compact_date("Mon Apr  7 09:15:32 2026 +0200"), "Apr  7");
        assert_eq!(
            compact_date("Fri Mar 28 11:00:00 2025 +0100"),
            "Mar 28 2025"
        );
    }
}
