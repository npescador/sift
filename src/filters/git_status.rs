use crate::filters::{FilterOutput, Verbosity};

/// Filter `git status` output into a compact grouped summary.
///
/// Groups files by state (staged / modified / untracked) with counts.
/// In Compact mode only shows counts + up to 3 representative filenames.
/// In Verbose mode shows all filenames per group.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if verbosity == Verbosity::Maximum {
        return FilterOutput::passthrough(raw);
    }

    let mut staged: Vec<&str> = Vec::new();
    let mut modified: Vec<&str> = Vec::new();
    let mut untracked: Vec<&str> = Vec::new();
    let mut branch_line = "";

    let mut current_section = Section::None;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("On branch") || trimmed.starts_with("HEAD detached") {
            branch_line = line;
            continue;
        }

        if trimmed == "Changes to be committed:" {
            current_section = Section::Staged;
            continue;
        }
        if trimmed == "Changes not staged for commit:" {
            current_section = Section::Modified;
            continue;
        }
        if trimmed == "Untracked files:" {
            current_section = Section::Untracked;
            continue;
        }
        if trimmed.is_empty()
            || trimmed.starts_with("(use \"git")
            || trimmed.starts_with("nothing to commit")
            || trimmed.starts_with("no changes added")
        {
            continue;
        }

        // Extract filename from lines like "  modified:   src/main.rs"
        let filename = trimmed
            .split_once(':')
            .map(|(_, f)| f.trim())
            .unwrap_or(trimmed);

        match current_section {
            Section::Staged => staged.push(filename),
            Section::Modified => modified.push(filename),
            Section::Untracked => untracked.push(filename),
            Section::None => {}
        }
    }

    if staged.is_empty() && modified.is_empty() && untracked.is_empty() {
        let content = format!("{branch_line}\nnothing to commit, working tree clean\n");
        let filtered_bytes = content.len();
        return FilterOutput {
            content,
            original_bytes,
            filtered_bytes,
        };
    }

    let max_files = match verbosity {
        Verbosity::Compact => 3,
        _ => usize::MAX,
    };

    let mut out = String::new();
    if !branch_line.is_empty() {
        out.push_str(branch_line);
        out.push('\n');
    }
    format_group(&mut out, "staged", &staged, max_files);
    format_group(&mut out, "modified", &modified, max_files);
    format_group(&mut out, "untracked", &untracked, max_files);

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

enum Section {
    None,
    Staged,
    Modified,
    Untracked,
}

fn format_group(out: &mut String, label: &str, files: &[&str], max_files: usize) {
    if files.is_empty() {
        return;
    }
    let shown = files.len().min(max_files);
    let remaining = files.len().saturating_sub(max_files);

    let file_list: Vec<&str> = files[..shown].to_vec();
    let names = file_list.join(", ");

    if remaining > 0 {
        out.push_str(&format!(
            "{label:10} {} files  ({}, +{} more)\n",
            files.len(),
            names,
            remaining
        ));
    } else {
        out.push_str(&format!(
            "{label:10} {} {}  ({})\n",
            files.len(),
            if files.len() == 1 { "file " } else { "files" },
            names
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STATUS: &str = "\
On branch main
Changes to be committed:
  (use \"git restore --staged <file>...\" to unstage)
\tmodified:   src/cli.rs

Changes not staged for commit:
  (use \"git add <file>...\" to update what will be committed)
\tmodified:   src/main.rs
\tmodified:   src/executor.rs

Untracked files:
  (use \"git add <file>...\" to include in what will be committed)
\tnotes.txt
";

    #[test]
    fn compact_shows_branch_and_counts() {
        let out = filter(SAMPLE_STATUS, Verbosity::Compact);
        assert!(out.content.contains("On branch main"));
        assert!(out.content.contains("staged"));
        assert!(out.content.contains("modified"));
        assert!(out.content.contains("untracked"));
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn maximum_returns_passthrough() {
        let out = filter(SAMPLE_STATUS, Verbosity::Maximum);
        assert_eq!(out.content, SAMPLE_STATUS);
    }

    #[test]
    fn clean_tree_shows_nothing_to_commit() {
        let clean = "On branch main\nnothing to commit, working tree clean\n";
        let out = filter(clean, Verbosity::Compact);
        assert!(out.content.contains("nothing to commit"));
    }
}
