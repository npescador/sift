use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Save raw command output to `~/.local/share/sift/raw/<timestamp>-<cmd>.txt`.
///
/// Called when a filter produces empty output from non-empty input — a possible
/// false negative. The raw file lets developers inspect what was silently dropped.
///
/// Returns the path where the file was saved, or `None` if writing failed.
/// Failures are silent — tee should never break the main command flow.
pub fn save_raw(cmd: &str, raw: &str) -> Option<PathBuf> {
    let dir = raw_dir()?;
    std::fs::create_dir_all(&dir).ok()?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let safe_cmd = sanitize_cmd(cmd);
    let filename = format!("{ts}-{safe_cmd}.txt");
    let path = dir.join(&filename);

    std::fs::write(&path, raw).ok()?;
    Some(path)
}

/// Resolve `~/.local/share/sift/raw/`, respecting `$XDG_DATA_HOME`.
fn raw_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("sift").join("raw"));
        }
    }
    let home = std::env::var("HOME").ok()?;
    Some(
        PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("sift")
            .join("raw"),
    )
}

/// Replace characters not safe for filenames with underscores.
fn sanitize_cmd(cmd: &str) -> String {
    cmd.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(40)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_cmd_replaces_spaces_and_slashes() {
        assert_eq!(sanitize_cmd("git log --graph"), "git_log_--graph");
        assert_eq!(sanitize_cmd("xcodebuild build"), "xcodebuild_build");
    }

    #[test]
    fn sanitize_cmd_truncates_at_40_chars() {
        let long = "a".repeat(60);
        assert_eq!(sanitize_cmd(&long).len(), 40);
    }

    #[test]
    fn sanitize_cmd_keeps_alphanumeric_and_dash() {
        assert_eq!(sanitize_cmd("my-cmd_v2"), "my-cmd_v2");
    }
}
