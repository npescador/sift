use crate::filters::{FilterOutput, Verbosity};

/// Filter `codesign` output.
///
/// Compact:
/// - `--verify`: show valid/invalid status + filename
/// - `-d`: show Identifier, TeamIdentifier, Format, Signature size
/// - Errors: show as-is
///
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();
    let key_fields = ["Identifier", "TeamIdentifier", "Format", "Signature size"];

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Verification result lines
        if trimmed.contains("valid on disk") || trimmed.contains("satisfies its Designated") {
            let path = trimmed.split(':').next().unwrap_or(trimmed);
            let fname = short_path(path);
            out.push_str(&format!("\x1b[32m✓\x1b[0m {fname}: valid\n"));
            continue;
        }

        if trimmed.contains("code object is not signed")
            || trimmed.contains("CSSMERR_")
            || trimmed.contains("failed to satisfy")
        {
            let path = trimmed.split(':').next().unwrap_or(trimmed);
            let fname = short_path(path);
            let msg = trimmed.split_once(": ").map(|(_, m)| m).unwrap_or(trimmed);
            out.push_str(&format!("\x1b[31m✗\x1b[0m {fname}: {msg}\n"));
            continue;
        }

        // Display fields (from -d output)
        if let Some(field_name) = key_fields.iter().find(|f| trimmed.starts_with(*f)) {
            let value = trimmed.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
            out.push_str(&format!("  {field_name}: {value}\n"));
            continue;
        }

        // Errors
        if trimmed.starts_with("codesign:") || trimmed.starts_with("error:") {
            out.push_str(&format!("\x1b[31m{trimmed}\x1b[0m\n"));
        }
    }

    if out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

/// Filter `security find-identity` output.
///
/// Compact: list identities with short hash (first 8 chars), name, and count.
/// VeryVerbose+: raw passthrough.
pub fn filter_security(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut out = String::new();
    let mut identity_count = 0usize;

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Count line: "N valid identities found"
        if trimmed.contains("valid identit") {
            out.push_str(&format!("{trimmed}\n"));
            continue;
        }

        // Identity lines: "  N) HASH "Name (Team)""
        if trimmed
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            if let Some(identity) = parse_identity_line(trimmed) {
                out.push_str(&format!("  {identity}\n"));
                identity_count += 1;
            }
            continue;
        }
    }

    // If we got nothing useful, passthrough
    if identity_count == 0 && out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
    }
}

/// Parse an identity line into "SHORT_HASH name".
fn parse_identity_line(line: &str) -> Option<String> {
    // "1) ABCDEF1234567890ABCDEF1234567890ABCDEF12 "Apple Development: dev@example.com (ABCDEF1234)""
    let after_num = line.split_once(')').map(|(_, rest)| rest.trim())?;
    let mut parts = after_num.splitn(2, ' ');
    let hash = parts.next()?.trim();
    let name = parts.next()?.trim().trim_matches('"');

    if hash.len() >= 8 {
        let short_hash = &hash[..8];
        Some(format!("{short_hash}…  {name}"))
    } else {
        Some(format!("{hash}  {name}"))
    }
}

fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 3 {
        return path.to_string();
    }
    parts[parts.len() - 3..].join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_VERIFY_VALID: &str = "\
/path/to/MyApp.app: valid on disk
/path/to/MyApp.app: satisfies its Designated Requirement
";

    const SAMPLE_DESCRIBE: &str = "\
Executable=/Users/dev/MyApp.app/Contents/MacOS/MyApp
Identifier=com.example.myapp
Format=app bundle with Mach-O universal (arm64 x86_64)
CodeDirectory v=20400 size=1234 flags=0x0(none) hashes=47+5 location=embedded
Signature size=4514
Timestamp=Apr  7, 2026 at 10:00:00
Info.plist entries=38; CodeResources entries=123
TeamIdentifier=ABCDEF1234
Sealed Resources version=2 rules=13 files=123
Internal requirements count=1 size=112
";

    const SAMPLE_INVALID: &str = "/path/to/MyApp.app: code object is not signed at all\n";

    const SAMPLE_SECURITY: &str = "\
  1) ABCDEF1234567890ABCDEF1234567890ABCDEF12 \"Apple Development: dev@example.com (ABCDEF1234)\"
  2) 1234567890ABCDEF1234567890ABCDEF12345678 \"Apple Distribution: Example Corp (DEFGH5678)\"
     2 valid identities found
";

    #[test]
    fn compact_verify_valid_shows_checkmark() {
        let out = filter(SAMPLE_VERIFY_VALID, Verbosity::Compact);
        assert!(out.content.contains('✓'));
        assert!(out.content.contains("valid"));
    }

    #[test]
    fn compact_describe_shows_key_fields() {
        let out = filter(SAMPLE_DESCRIBE, Verbosity::Compact);
        assert!(out.content.contains("Identifier"));
        assert!(out.content.contains("com.example.myapp"));
        assert!(out.content.contains("TeamIdentifier"));
        assert!(out.content.contains("Signature size"));
    }

    #[test]
    fn compact_describe_strips_noise() {
        let out = filter(SAMPLE_DESCRIBE, Verbosity::Compact);
        assert!(!out.content.contains("CodeDirectory"));
        assert!(!out.content.contains("Info.plist entries"));
    }

    #[test]
    fn compact_invalid_shows_cross() {
        let out = filter(SAMPLE_INVALID, Verbosity::Compact);
        assert!(out.content.contains('✗'));
    }

    #[test]
    fn very_verbose_returns_passthrough_codesign() {
        let out = filter(SAMPLE_VERIFY_VALID, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_VERIFY_VALID);
    }

    #[test]
    fn security_shows_identities() {
        let out = filter_security(SAMPLE_SECURITY, Verbosity::Compact);
        assert!(out.content.contains("ABCDEF12…") || out.content.contains("ABCDEF1234"));
        assert!(out.content.contains("Apple Development"));
    }

    #[test]
    fn security_shows_count() {
        let out = filter_security(SAMPLE_SECURITY, Verbosity::Compact);
        assert!(out.content.contains("valid identit"));
    }

    #[test]
    fn security_very_verbose_passthrough() {
        let out = filter_security(SAMPLE_SECURITY, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_SECURITY);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_DESCRIBE, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}
