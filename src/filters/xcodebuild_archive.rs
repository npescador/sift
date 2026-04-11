use crate::filters::types::{Diagnostic, Severity, XcodebuildArchiveResult};
use crate::filters::{FilterOutput, Verbosity};

pub fn parse(raw: &str) -> XcodebuildArchiveResult {
    let succeeded = raw.contains("** ARCHIVE SUCCEEDED **");
    let errors_map = collect_errors(raw);
    let warnings_count = raw.lines().filter(|l| l.contains(": warning:")).count();
    let archive_path = extract_archive_path(raw);
    let scheme = extract_flag(raw, "-scheme");
    let configuration = extract_flag(raw, "-configuration");
    let team = extract_signing_team(raw);
    let identity = extract_signing_identity(raw);

    let errors: Vec<Diagnostic> = errors_map
        .into_iter()
        .flat_map(|(file, messages)| {
            messages.into_iter().map(move |msg| Diagnostic {
                file: file.clone(),
                line: None,
                column: None,
                severity: Severity::Error,
                message: msg,
            })
        })
        .collect();

    XcodebuildArchiveResult {
        succeeded,
        archive_path,
        scheme,
        configuration,
        team,
        identity,
        errors,
        warnings_count,
    }
}

/// Filter `xcodebuild archive` output.
///
/// Compact: result header + scheme/config + archive path + signing info + errors.
/// Verbose: adds compile warnings count and intermediate step progress.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);
    let errors_map = collect_errors(raw);

    let mut out = String::new();

    if result.succeeded {
        out.push_str("\x1b[32mARCHIVE SUCCEEDED\x1b[0m");
    } else if raw.contains("** ARCHIVE FAILED **") {
        out.push_str("\x1b[31mARCHIVE FAILED\x1b[0m");
    } else {
        out.push_str("ARCHIVE");
    }

    match (&result.scheme, &result.configuration) {
        (Some(s), Some(c)) => out.push_str(&format!("  {s}  [{c}]\n")),
        (Some(s), None) => out.push_str(&format!("  {s}\n")),
        _ => out.push('\n'),
    }

    if let Some(path) = &result.archive_path {
        let short = shorten_path(path);
        out.push_str(&format!("  📦 {short}\n"));
    }

    if let Some(t) = &result.team {
        out.push_str(&format!("  🔑 Team: {t}\n"));
    }
    if let Some(id) = &result.identity {
        out.push_str(&format!("  🔐 {id}\n"));
    }

    if matches!(verbosity, Verbosity::Verbose) && result.warnings_count > 0 {
        out.push_str(&format!(
            "  ⚠  {} warning{}\n",
            result.warnings_count,
            if result.warnings_count == 1 { "" } else { "s" }
        ));
    }

    if !errors_map.is_empty() {
        out.push('\n');
        for (file, messages) in &errors_map {
            let short_file = shorten_path(file);
            out.push_str(&format!(
                "{short_file} ({} error{})\n",
                messages.len(),
                if messages.len() == 1 { "" } else { "s" }
            ));
            for msg in messages {
                out.push_str(&format!("  error: {msg}\n"));
            }
        }
    }

    FilterOutput {
        filtered_bytes: out.len(),
        content: out,
        original_bytes,
        structured: serde_json::to_value(&result).ok(),
    }
}

fn collect_errors(raw: &str) -> std::collections::BTreeMap<String, Vec<String>> {
    let mut map: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for line in raw.lines() {
        if line.contains(": error:") {
            let (file, msg) = split_diagnostic(line);
            map.entry(file).or_default().push(msg);
        }
    }
    map
}

fn split_diagnostic(line: &str) -> (String, String) {
    if let Some(idx) = line.find(": error:") {
        let path_part = &line[..idx];
        let msg_part = line[idx + 8..].trim().to_string();
        let file = path_part
            .rsplitn(3, ':')
            .last()
            .unwrap_or(path_part)
            .to_string();
        return (file, msg_part);
    }
    (String::new(), line.to_string())
}

fn extract_flag(raw: &str, flag: &str) -> Option<String> {
    for line in raw.lines().take(30) {
        let line = line.trim();
        if let Some(pos) = line.find(flag) {
            let rest = line[pos + flag.len()..].trim_start_matches('=').trim();
            let value: String = rest
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches('"')
                .to_string();
            if !value.is_empty() && !value.starts_with('-') {
                return Some(value);
            }
        }
    }
    for line in raw.lines() {
        if line.trim().starts_with("SCHEME") || line.trim().starts_with("scheme") {
            if let Some(val) = line.split('=').nth(1) {
                let v = val.trim().to_string();
                if !v.is_empty() {
                    return Some(v);
                }
            }
        }
    }
    None
}

fn extract_archive_path(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("Archive saved at") {
            return trimmed
                .split_once(" at ")
                .map(|(_, p)| p.trim().to_string());
        }
        if trimmed.starts_with("ARCHIVE_PRODUCTS_PATH") {
            if let Some(val) = trimmed.split('=').nth(1) {
                return Some(val.trim().to_string());
            }
        }
        if trimmed.ends_with(".xcarchive") && trimmed.starts_with('/') {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_signing_team(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("Team:") || t.starts_with("DEVELOPMENT_TEAM") {
            let val = t
                .split_once('=')
                .map(|(_, v)| v)
                .or_else(|| t.split_once(':').map(|(_, v)| v))?
                .trim()
                .to_string();
            if !val.is_empty() {
                return Some(val);
            }
        }
    }
    None
}

fn extract_signing_identity(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("Signing Identity:") {
            return t.split_once(':').map(|(_, v)| v.trim().to_string());
        }
        if t.contains("Apple Distribution:") || t.contains("iPhone Distribution:") {
            let start = t
                .find("Apple Distribution:")
                .or_else(|| t.find("iPhone Distribution:"))
                .unwrap_or(0);
            let snippet: String = t[start..].chars().take(60).collect();
            return Some(snippet);
        }
    }
    None
}

fn shorten_path(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    let p = if !home.is_empty() {
        path.replacen(&home, "~", 1)
    } else {
        path.to_string()
    };
    let parts: Vec<&str> = p.split('/').collect();
    if parts.len() > 4 {
        format!("…/{}", parts[parts.len() - 3..].join("/"))
    } else {
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_success() -> &'static str {
        "Build settings from command line:\n    \
         -scheme MyApp -configuration Release\n\
         \n\
         === BUILD TARGET MyApp ===\n\
         CompileSwift normal arm64 AppDelegate.swift\n\
         Signing Identity:     Apple Distribution: Acme Corp (ABC123XYZ)\n\
         Team:                 Acme Corp (ABC123XYZ)\n\
         \n\
         Archive saved at /Users/dev/Library/Developer/Xcode/Archives/2026-04-07/MyApp.xcarchive\n\
         \n\
         ** ARCHIVE SUCCEEDED **\n"
    }

    fn sample_failed() -> &'static str {
        "Build settings from command line:\n    \
         -scheme MyApp -configuration Release\n\
         \n\
         /Users/dev/MyApp/Sources/PaymentService.swift:42:5: error: use of unresolved identifier 'PaymentResult'\n\
         /Users/dev/MyApp/Sources/PaymentService.swift:55:12: error: cannot convert value of type 'String' to 'Amount'\n\
         /Users/dev/MyApp/Sources/NetworkClient.swift:18:3: error: value of type 'URLSession' has no member 'dataTaskAsync'\n\
         \n\
         ** ARCHIVE FAILED **\n"
    }

    fn sample_with_warnings() -> &'static str {
        "Build settings from command line:\n    -scheme MyApp\n\
         /Users/dev/MyApp/Sources/Helper.swift:10:3: warning: result of call is unused\n\
         /Users/dev/MyApp/Sources/Helper.swift:20:3: warning: deprecated\n\
         Archive saved at /Users/dev/Library/Developer/Xcode/Archives/MyApp.xcarchive\n\
         ** ARCHIVE SUCCEEDED **\n"
    }

    #[test]
    fn success_shows_succeeded_header() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.content.contains("ARCHIVE SUCCEEDED"));
    }

    #[test]
    fn failure_shows_failed_header() {
        let out = filter(sample_failed(), Verbosity::Compact);
        assert!(out.content.contains("ARCHIVE FAILED"));
    }

    #[test]
    fn success_shows_archive_path() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.content.contains("MyApp.xcarchive"));
    }

    #[test]
    fn success_shows_signing_identity() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.content.contains("Apple Distribution"));
    }

    #[test]
    fn failure_shows_errors_grouped_by_file() {
        let out = filter(sample_failed(), Verbosity::Compact);
        assert!(out.content.contains("PaymentService.swift"));
        assert!(out.content.contains("NetworkClient.swift"));
        assert!(out.content.contains("2 error"));
    }

    #[test]
    fn verbose_shows_warning_count() {
        let out = filter(sample_with_warnings(), Verbosity::Verbose);
        assert!(out.content.contains("2 warning"));
    }

    #[test]
    fn compact_does_not_show_warning_count() {
        let out = filter(sample_with_warnings(), Verbosity::Compact);
        assert!(!out.content.contains("warning"));
    }

    #[test]
    fn very_verbose_is_passthrough() {
        let out = filter(sample_success(), Verbosity::VeryVerbose);
        assert_eq!(out.content, sample_success());
    }

    #[test]
    fn bytes_significantly_reduced_on_success() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn shorten_path_keeps_last_three_components() {
        let p = "/a/b/c/d/e/f/MyApp.xcarchive";
        let short = shorten_path(p);
        assert!(short.contains("MyApp.xcarchive"));
        assert!(short.starts_with('…'));
    }

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(sample_success());
        assert!(result.succeeded);
        assert!(result.archive_path.is_some());
        assert!(result.identity.is_some());
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(sample_success(), Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
