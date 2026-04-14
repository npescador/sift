//! `sift update` — self-update from the latest GitHub release.
//!
//! Uses `curl` (always available on macOS/Linux) to:
//!   1. Fetch the latest release tag from the GitHub API
//!   2. Compare with the running version
//!   3. Download the new binary and atomically replace the current executable

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::SiftError;

const REPO: &str = "npescador/sift";
const GITHUB_API: &str = "https://api.github.com/repos/npescador/sift/releases/latest";

/// Result of checking for an update.
pub enum UpdateCheck {
    /// Already on the latest version.
    UpToDate { version: String },
    /// A newer version is available.
    Available { current: String, latest: String },
}

/// Check the latest release tag on GitHub and compare with the running version.
///
/// Returns `Err` if the network request fails or the response cannot be parsed.
pub fn check_latest() -> Result<UpdateCheck, SiftError> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let latest = fetch_latest_tag()?;
    let latest = latest.trim_start_matches('v').to_string();

    if latest == current || version_less_or_equal(&latest, &current) {
        Ok(UpdateCheck::UpToDate { version: current })
    } else {
        Ok(UpdateCheck::Available { current, latest })
    }
}

/// Download the latest binary and replace the running executable.
///
/// The replacement is atomic: the new binary is written to a temp file first,
/// then moved over the original path.
pub fn perform_update(latest_version: &str) -> Result<PathBuf, SiftError> {
    let target = detect_target();
    let asset_name = format!("sift-{target}.tar.gz");
    let url = format!("https://github.com/{REPO}/releases/download/v{latest_version}/{asset_name}");

    let exe_path = current_exe_path()?;
    let tmp_archive = exe_path.with_extension("update.tar.gz");
    let tmp_bin = exe_path.with_extension("update.new");

    // Download archive
    curl_download(&url, &tmp_archive)?;

    // Extract the `sift` binary from the tarball
    let status = Command::new("tar")
        .args([
            "xzf",
            &tmp_archive.to_string_lossy(),
            "-C",
            &tmp_bin
                .parent()
                .unwrap_or(Path::new("/tmp"))
                .to_string_lossy(),
            "sift",
        ])
        .status()
        .map_err(SiftError::Io)?;

    let _ = fs::remove_file(&tmp_archive);

    if !status.success() {
        return Err(SiftError::Io(io::Error::other("tar extraction failed")));
    }

    // The extracted binary lands at <parent>/sift — move it to the tmp path
    let extracted = exe_path.parent().unwrap_or(Path::new("/tmp")).join("sift");
    if extracted != tmp_bin && extracted.exists() {
        fs::rename(&extracted, &tmp_bin).map_err(SiftError::Io)?;
    }

    // Make executable and atomically replace
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if tmp_bin.exists() {
            let mut perms = fs::metadata(&tmp_bin).map_err(SiftError::Io)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&tmp_bin, perms).map_err(SiftError::Io)?;
        }
    }

    if tmp_bin.exists() {
        fs::rename(&tmp_bin, &exe_path).map_err(SiftError::Io)?;
    }

    Ok(exe_path)
}

/// Run the full update flow and print progress to stdout.
pub fn run(check_only: bool) -> Result<i32, crate::error::SiftError> {
    println!("Checking for updates...");

    match check_latest()? {
        UpdateCheck::UpToDate { version } => {
            println!("sift {version} is already up to date.");
            Ok(0)
        }
        UpdateCheck::Available { current, latest } => {
            println!("Update available: {current} → {latest}");

            if check_only {
                println!("Run `sift update` (without --check) to install.");
                return Ok(0);
            }

            println!("Downloading sift {latest}...");
            match perform_update(&latest) {
                Ok(path) => {
                    println!("✓ Updated to {latest} at {}", path.display());
                    Ok(0)
                }
                Err(e) => {
                    eprintln!("[sift update] failed: {e}");
                    eprintln!(
                        "You can manually download from: https://github.com/{REPO}/releases/latest"
                    );
                    Ok(1)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Fetch the latest release tag string from the GitHub API using `curl`.
fn fetch_latest_tag() -> Result<String, SiftError> {
    let output = Command::new("curl")
        .args([
            "--silent",
            "--fail",
            "--location",
            "--max-time",
            "10",
            "--user-agent",
            &format!("sift-cli/{}", env!("CARGO_PKG_VERSION")),
            GITHUB_API,
        ])
        .output()
        .map_err(SiftError::Io)?;

    if !output.status.success() {
        return Err(SiftError::Io(io::Error::other(format!(
            "GitHub API request failed (exit {})",
            output.status.code().unwrap_or(-1)
        ))));
    }

    let body = String::from_utf8_lossy(&output.stdout);
    parse_tag_name(&body).ok_or_else(|| {
        SiftError::Io(io::Error::new(
            io::ErrorKind::InvalidData,
            "could not parse tag_name from GitHub API response",
        ))
    })
}

/// Extract `tag_name` from a minimal GitHub releases JSON response.
///
/// Avoids pulling in `serde_json` — parses the single field we need.
pub fn parse_tag_name(json: &str) -> Option<String> {
    // Look for "tag_name":"v0.7.0" or "tag_name": "v0.7.0"
    let key = "\"tag_name\"";
    let pos = json.find(key)?;
    let after_key = &json[pos + key.len()..];
    let colon = after_key.find(':')?;
    let after_colon = after_key[colon + 1..].trim_start();
    if after_colon.starts_with('"') {
        let inner = after_colon.strip_prefix('"')?;
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        None
    }
}

/// Download a URL to a local path using `curl`.
fn curl_download(url: &str, dest: &Path) -> Result<(), SiftError> {
    let status = Command::new("curl")
        .args([
            "--silent",
            "--fail",
            "--location",
            "--max-time",
            "60",
            "--output",
            &dest.to_string_lossy(),
            url,
        ])
        .status()
        .map_err(SiftError::Io)?;

    if status.success() {
        Ok(())
    } else {
        Err(SiftError::Io(io::Error::other(format!(
            "curl download failed for {url} (exit {})",
            status.code().unwrap_or(-1)
        ))))
    }
}

/// Detect the target triple for asset naming (macOS arm64/x86_64, Linux x86_64).
fn detect_target() -> String {
    let arch = std::env::consts::ARCH;
    let os = std::env::consts::OS;
    match (os, arch) {
        ("macos", "aarch64") => "aarch64-apple-darwin".to_string(),
        ("macos", "x86_64") => "x86_64-apple-darwin".to_string(),
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu".to_string(),
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu".to_string(),
        _ => format!("{arch}-{os}"),
    }
}

/// Return the path of the currently running sift executable.
fn current_exe_path() -> Result<PathBuf, SiftError> {
    env::current_exe().map_err(SiftError::Io)
}

/// Returns true if `a` is less than or equal to `b` using semver-style comparison.
/// Falls back to string equality if parsing fails.
pub fn version_less_or_equal(a: &str, b: &str) -> bool {
    if let (Some(av), Some(bv)) = (parse_semver(a), parse_semver(b)) {
        av <= bv
    } else {
        a <= b
    }
}

/// Parse "MAJOR.MINOR.PATCH" into a comparable tuple.
pub fn parse_semver(v: &str) -> Option<(u32, u32, u32)> {
    let v = v.trim_start_matches('v');
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() < 3 {
        return None;
    }
    let major = parts[0].parse().ok()?;
    let minor = parts[1].parse().ok()?;
    let patch = parts[2].split('-').next()?.parse().ok()?;
    Some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tag_name_standard() {
        let json = r#"{"tag_name":"v0.7.0","name":"v0.7.0 Release"}"#;
        assert_eq!(parse_tag_name(json).as_deref(), Some("v0.7.0"));
    }

    #[test]
    fn parse_tag_name_with_spaces() {
        let json = r#"{"tag_name": "v1.0.0", "draft": false}"#;
        assert_eq!(parse_tag_name(json).as_deref(), Some("v1.0.0"));
    }

    #[test]
    fn parse_tag_name_missing_returns_none() {
        let json = r#"{"name": "something", "draft": false}"#;
        assert_eq!(parse_tag_name(json), None);
    }

    #[test]
    fn parse_semver_valid() {
        assert_eq!(parse_semver("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_semver("v0.7.0"), Some((0, 7, 0)));
        assert_eq!(parse_semver("1.0.0-beta"), Some((1, 0, 0)));
    }

    #[test]
    fn parse_semver_invalid_returns_none() {
        assert_eq!(parse_semver("notaversion"), None);
        assert_eq!(parse_semver("1.2"), None);
    }

    #[test]
    fn version_less_or_equal_comparisons() {
        assert!(version_less_or_equal("0.6.0", "0.7.0"));
        assert!(version_less_or_equal("0.7.0", "0.7.0"));
        assert!(!version_less_or_equal("0.8.0", "0.7.0"));
        assert!(version_less_or_equal("1.0.0", "2.0.0"));
    }

    #[test]
    fn detect_target_returns_non_empty() {
        assert!(!detect_target().is_empty());
    }
}
