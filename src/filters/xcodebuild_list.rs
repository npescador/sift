use crate::filters::{FilterOutput, Verbosity};

/// Filter `xcodebuild -list` output.
///
/// Compact: project name + schemes (first = default) + configurations.
/// Verbose: adds full targets list.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let original_bytes = raw.len();

    let project = extract_project(raw);
    let schemes = extract_section(raw, "Schemes:");
    let targets = extract_section(raw, "Targets:");
    let configs = extract_section(raw, "Build Configurations:");
    let default_scheme = extract_default_scheme(raw);

    let mut out = String::new();

    // Header
    match &project {
        Some(p) => out.push_str(&format!("📋 {p}\n")),
        None => out.push_str("📋 xcodebuild -list\n"),
    }

    // Schemes — mark the default
    if !schemes.is_empty() {
        out.push_str(&format!("\nSchemes ({}):\n", schemes.len()));
        for s in &schemes {
            let marker = if default_scheme.as_deref() == Some(s.as_str()) {
                "  ★ "
            } else {
                "    "
            };
            out.push_str(&format!("{marker}{s}\n"));
        }
    }

    // Configurations (compact: always shown)
    if !configs.is_empty() {
        out.push_str(&format!("\nConfigurations: {}\n", configs.join("  |  ")));
    }

    // Targets — verbose only
    if matches!(verbosity, Verbosity::Verbose) && !targets.is_empty() {
        out.push_str(&format!("\nTargets ({}):\n", targets.len()));
        for t in &targets {
            out.push_str(&format!("    {t}\n"));
        }
    } else if !targets.is_empty() {
        out.push_str(&format!(
            "\nTargets: {} (use -v for full list)\n",
            targets.len()
        ));
    }

    FilterOutput {
        filtered_bytes: out.len(),
        content: out,
        original_bytes,
        structured: None,
    }
}

/// Extract project name from "Information about project \"Name\":"
fn extract_project(raw: &str) -> Option<String> {
    for line in raw.lines() {
        let t = line.trim();
        if t.starts_with("Information about project") {
            if let Some(start) = t.find('"') {
                if let Some(end) = t[start + 1..].find('"') {
                    return Some(t[start + 1..start + 1 + end].to_string());
                }
            }
        }
        // Workspace variant: "Information about workspace \"Name\":"
        if t.starts_with("Information about workspace") {
            if let Some(start) = t.find('"') {
                if let Some(end) = t[start + 1..].find('"') {
                    return Some(format!("{} (workspace)", &t[start + 1..start + 1 + end]));
                }
            }
        }
    }
    None
}

/// Extract items from an indented section like:
/// ```text
/// Schemes:
///     MyApp
///     MyApp-Dev
/// ```
fn extract_section(raw: &str, section: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut in_section = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed == section.trim_end_matches(':') || trimmed.trim_end() == section {
            in_section = true;
            continue;
        }
        if in_section {
            if trimmed.is_empty() {
                break;
            }
            if !line.starts_with("    ") && !line.starts_with('\t') {
                break;
            }
            items.push(trimmed.to_string());
        }
    }
    items
}

/// Extract "If no scheme is specified and -list is not passed then xcodebuild will build
/// the scheme: MyApp" — the default scheme mentioned at the end of -list output.
fn extract_default_scheme(raw: &str) -> Option<String> {
    for line in raw.lines() {
        if line.contains("xcodebuild will build the scheme") {
            // "...will build the scheme: MyApp"
            if let Some(idx) = line.rfind(':') {
                let val = line[idx + 1..].trim().to_string();
                if !val.is_empty() {
                    return Some(val);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        "Information about project \"MyApp\":\n    \
         Targets:\n        \
         MyApp\n        \
         MyAppTests\n        \
         MyAppUITests\n\n    \
         Build Configurations:\n        \
         Debug\n        \
         Release\n\n    \
         If no build configuration is provided, xcodebuild will use Release.\n\n    \
         Schemes:\n        \
         MyApp\n        \
         MyApp-Dev\n        \
         MyApp-Staging\n\n    \
         If no scheme is specified and -list is not passed then xcodebuild will build the scheme: MyApp\n"
    }

    fn sample_workspace() -> &'static str {
        "Information about workspace \"MyApp\":\n    \
         Schemes:\n        \
         MyApp\n        \
         MyApp-Dev\n"
    }

    #[test]
    fn extracts_project_name() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.content.contains("MyApp"));
    }

    #[test]
    fn shows_scheme_count() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.content.contains("Schemes (3)"));
    }

    #[test]
    fn marks_default_scheme() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.content.contains("★"));
    }

    #[test]
    fn shows_configurations() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.content.contains("Debug"));
        assert!(out.content.contains("Release"));
    }

    #[test]
    fn compact_shows_target_count_not_list() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.content.contains("Targets: 3"));
        assert!(!out.content.contains("MyAppTests\n"));
    }

    #[test]
    fn verbose_shows_full_targets() {
        let out = filter(sample(), Verbosity::Verbose);
        assert!(out.content.contains("MyAppTests"));
        assert!(out.content.contains("MyAppUITests"));
    }

    #[test]
    fn workspace_variant_detected() {
        let out = filter(sample_workspace(), Verbosity::Compact);
        assert!(out.content.contains("workspace"));
    }

    #[test]
    fn very_verbose_is_passthrough() {
        let out = filter(sample(), Verbosity::VeryVerbose);
        assert_eq!(out.content, sample());
    }

    #[test]
    fn bytes_significantly_reduced() {
        let out = filter(sample(), Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }

    #[test]
    fn extract_section_parses_correctly() {
        let schemes = extract_section(sample(), "Schemes:");
        assert_eq!(schemes, vec!["MyApp", "MyApp-Dev", "MyApp-Staging"]);
    }
}
