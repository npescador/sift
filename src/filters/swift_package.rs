use crate::filters::{FilterOutput, Verbosity};

/// Filter `swift package resolve` / `update` / `show-dependencies` output.
///
/// Compact: one line per package — name + version.
/// Verbose: adds source URL.
/// VeryVerbose+: raw passthrough.
pub fn filter(input: &str, verbosity: Verbosity) -> FilterOutput {
    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(input);
    }

    let original_bytes = input.len();
    let is_update = input.contains("Updating") || input.contains("updating");
    let operation = if is_update { "Updated" } else { "Resolved" };

    let packages = parse_packages(input);
    let errors = collect_errors(input);

    let mut out = String::new();

    if !errors.is_empty() {
        out.push_str("❌ swift package failed\n\n");
        for e in &errors {
            out.push_str(&format!("  {e}\n"));
        }
        return FilterOutput {
            filtered_bytes: out.len(),
            content: out,
            original_bytes,
        };
    }

    if packages.is_empty() {
        out.push_str("✓ swift package — nothing to do\n");
        return FilterOutput {
            filtered_bytes: out.len(),
            content: out,
            original_bytes,
        };
    }

    out.push_str(&format!(
        "📦 {} {} package{}\n",
        operation,
        packages.len(),
        if packages.len() == 1 { "" } else { "s" }
    ));

    for pkg in &packages {
        if matches!(verbosity, Verbosity::Verbose) && !pkg.url.is_empty() {
            out.push_str(&format!(
                "  {:<32} {}  {}\n",
                pkg.name, pkg.version, pkg.url
            ));
        } else {
            out.push_str(&format!("  {:<32} {}\n", pkg.name, pkg.version));
        }
    }

    FilterOutput {
        filtered_bytes: out.len(),
        content: out,
        original_bytes,
    }
}

#[derive(Debug)]
struct PackageInfo {
    name: String,
    version: String,
    url: String,
}

/// Parse packages from SPM resolve/update/show-dependencies output.
fn parse_packages(input: &str) -> Vec<PackageInfo> {
    let mut map: std::collections::BTreeMap<String, PackageInfo> =
        std::collections::BTreeMap::new();

    for line in input.lines() {
        let trimmed = line.trim();

        // "Fetched https://github.com/org/name (1.2.3)" or from cache
        if trimmed.starts_with("Fetched ") || trimmed.starts_with("Fetching ") {
            if let Some(url) = extract_url(trimmed) {
                let name = url_to_name(&url);
                let ver = extract_parenthetical_version(trimmed).unwrap_or_default();
                map.entry(name.clone())
                    .and_modify(|p| {
                        if p.version.is_empty() && !ver.is_empty() {
                            p.version = ver.clone();
                        }
                        if p.url.is_empty() {
                            p.url = url.clone();
                        }
                    })
                    .or_insert(PackageInfo {
                        name,
                        version: ver,
                        url,
                    });
            }
            continue;
        }

        // "Updating https://github.com/org/name to 1.2.3"
        if trimmed.starts_with("Updating ") {
            if let Some(url) = extract_url(trimmed) {
                let name = url_to_name(&url);
                let ver = trimmed
                    .split(" to ")
                    .nth(1)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_default();
                map.entry(name.clone())
                    .and_modify(|p| {
                        if !ver.is_empty() {
                            p.version = ver.clone();
                        }
                    })
                    .or_insert(PackageInfo {
                        name,
                        version: ver,
                        url,
                    });
            }
            continue;
        }

        // "name @ version" — from show-dependencies JSON or plain text
        if trimmed.contains(" @ ") && !trimmed.contains("://") {
            if let Some((name, ver)) = trimmed.split_once(" @ ") {
                let name = name.trim().to_string();
                let ver = ver.trim().to_string();
                map.entry(name.clone())
                    .and_modify(|p| p.version = ver.clone())
                    .or_insert(PackageInfo {
                        name,
                        version: ver,
                        url: String::new(),
                    });
            }
            continue;
        }

        // Tree lines from show-dependencies: "└── swift-argument-parser 1.3.0"
        if trimmed.starts_with("└──") || trimmed.starts_with("├──") || trimmed.starts_with('│')
        {
            let clean = trimmed.trim_start_matches(|c: char| {
                matches!(c, '\u{2514}' | '\u{251C}' | '\u{2500}' | '\u{2502}' | ' ')
            });
            let parts: Vec<&str> = clean.splitn(2, ' ').collect();
            if parts.len() == 2 {
                let name = parts[0].to_string();
                let ver = parts[1].trim().to_string();
                map.entry(name.clone())
                    .and_modify(|p| {
                        if p.version.is_empty() {
                            p.version = ver.clone();
                        }
                    })
                    .or_insert(PackageInfo {
                        name,
                        version: ver,
                        url: String::new(),
                    });
            }
        }
    }

    map.into_values().collect()
}

/// Extract errors from SPM output.
fn collect_errors(input: &str) -> Vec<String> {
    input
        .lines()
        .filter(|l| {
            let t = l.trim();
            t.starts_with("error:") || (t.contains(": error:") && !t.contains("warning:"))
        })
        .map(|l| l.trim().to_string())
        .collect()
}

/// Extract first https:// URL from a line.
fn extract_url(line: &str) -> Option<String> {
    let start = line.find("https://")?;
    let rest = &line[start..];
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Extract "(1.2.3)" version from end of a line.
fn extract_parenthetical_version(line: &str) -> Option<String> {
    let start = line.rfind('(')?;
    let end = line.rfind(')')?;
    if end > start {
        Some(line[start + 1..end].to_string())
    } else {
        None
    }
}

/// Turn a GitHub URL into a short package name.
/// "https://github.com/apple/swift-argument-parser" -> "swift-argument-parser"
fn url_to_name(url: &str) -> String {
    url.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .trim_end_matches(".git")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_resolve() -> &'static str {
        "Fetching https://github.com/apple/swift-argument-parser from cache\n\
         Fetched https://github.com/apple/swift-argument-parser (1.3.0)\n\
         Fetching https://github.com/nicklockwood/SwiftFormat from cache\n\
         Fetched https://github.com/nicklockwood/SwiftFormat (0.53.5)\n\
         Fetching https://github.com/realm/SwiftLint from cache\n\
         Fetched https://github.com/realm/SwiftLint (0.54.0)\n"
    }

    fn sample_update() -> &'static str {
        "Updating https://github.com/apple/swift-argument-parser to 1.4.0\n\
         Updating https://github.com/nicklockwood/SwiftFormat to 0.54.0\n"
    }

    fn sample_nothing() -> &'static str {
        "Everything is already up-to-date.\n"
    }

    fn sample_error() -> &'static str {
        "error: no such module 'ArgumentParser'\n\
         error: manifest parse error(s)\n"
    }

    fn sample_show_dependencies() -> &'static str {
        ".\n\
         └── swift-argument-parser 1.3.0\n\
         └── swift-format 509.0.0\n"
    }

    #[test]
    fn resolve_shows_package_count() {
        let out = filter(sample_resolve(), Verbosity::Compact);
        assert!(out.content.contains("Resolved"));
        assert!(out.content.contains("3 packages"));
    }

    #[test]
    fn resolve_shows_package_names() {
        let out = filter(sample_resolve(), Verbosity::Compact);
        assert!(out.content.contains("swift-argument-parser"));
        assert!(out.content.contains("SwiftFormat"));
        assert!(out.content.contains("SwiftLint"));
    }

    #[test]
    fn resolve_shows_versions() {
        let out = filter(sample_resolve(), Verbosity::Compact);
        assert!(out.content.contains("1.3.0"));
        assert!(out.content.contains("0.53.5"));
    }

    #[test]
    fn update_shows_updated_header() {
        let out = filter(sample_update(), Verbosity::Compact);
        assert!(out.content.contains("Updated"));
    }

    #[test]
    fn nothing_to_do_shows_checkmark() {
        let out = filter(sample_nothing(), Verbosity::Compact);
        assert!(out.content.contains("nothing to do"));
    }

    #[test]
    fn errors_show_failure_header() {
        let out = filter(sample_error(), Verbosity::Compact);
        assert!(out.content.contains("❌"));
        assert!(out.content.contains("failed"));
    }

    #[test]
    fn show_dependencies_parses_tree() {
        let out = filter(sample_show_dependencies(), Verbosity::Compact);
        assert!(out.content.contains("swift-argument-parser"));
        assert!(out.content.contains("swift-format"));
    }

    #[test]
    fn verbose_shows_url() {
        let out = filter(sample_resolve(), Verbosity::Verbose);
        assert!(out.content.contains("github.com"));
    }

    #[test]
    fn compact_does_not_show_url() {
        let out = filter(sample_resolve(), Verbosity::Compact);
        assert!(!out.content.contains("github.com"));
    }

    #[test]
    fn very_verbose_is_passthrough() {
        let out = filter(sample_resolve(), Verbosity::VeryVerbose);
        assert_eq!(out.content, sample_resolve());
    }

    #[test]
    fn bytes_significantly_reduced() {
        let out = filter(sample_resolve(), Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn url_to_name_strips_org_and_git() {
        assert_eq!(
            url_to_name("https://github.com/apple/swift-argument-parser"),
            "swift-argument-parser"
        );
        assert_eq!(
            url_to_name("https://github.com/realm/SwiftLint.git"),
            "SwiftLint"
        );
    }
}
