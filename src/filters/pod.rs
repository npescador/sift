use crate::filters::{FilterOutput, Verbosity};

/// Filter `pod install` / `pod update` output.
///
/// Compact:
/// - Result line: `✓ Pod install complete — N pods installed` or `✗ Pod install failed`
/// - List installed pods: `  PodName version`
/// - Show `[!]` warning/error lines always
/// - Strip "Analyzing dependencies", "Downloading dependencies",
///   "Generating Pods project", "Integrating client project"
///
/// Verbose: same + show "Using" (unchanged) pods too.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let mut installed_pods: Vec<String> = Vec::new();
    let mut using_pods: Vec<String> = Vec::new();
    let mut notices: Vec<String> = Vec::new();
    let mut completion_line: Option<String> = None;
    let mut failed = false;

    let noise = [
        "Analyzing dependencies",
        "Downloading dependencies",
        "Generating Pods project",
        "Integrating client project",
        "Fetching podspec for",
        "Resolving dependencies of target",
    ];

    for line in raw.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Warnings and errors
        if trimmed.starts_with("[!]") {
            notices.push(trimmed.to_string());
            continue;
        }

        // Completion line
        if trimmed.starts_with("Pod installation complete") || trimmed.contains("pods installed") {
            completion_line = Some(trimmed.to_string());
            continue;
        }

        // Error indicators
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("[!] Unable to")
        {
            failed = true;
            notices.push(trimmed.to_string());
            continue;
        }

        // Skip noise lines
        if noise.iter().any(|n| trimmed.starts_with(n)) {
            continue;
        }

        // Installing PodName (version)
        if let Some(rest) = trimmed.strip_prefix("Installing ") {
            if let Some(pod) = parse_pod_line(rest) {
                installed_pods.push(pod);
            }
            continue;
        }

        // Using PodName (version) — unchanged pods
        if let Some(rest) = trimmed.strip_prefix("Using ") {
            if let Some(pod) = parse_pod_line(rest) {
                using_pods.push(pod);
            }
        }
    }

    let mut out = String::new();

    // Result header
    if let Some(ref completion) = completion_line {
        let pod_count = installed_pods.len() + using_pods.len();
        let action = if using_pods.is_empty() {
            "install"
        } else {
            "update"
        };
        // Extract pod count from completion line if possible
        let count_str = extract_pod_count(completion).unwrap_or(pod_count);
        out.push_str(&format!(
            "\x1b[32m✓\x1b[0m Pod {action} complete — {count_str} pod{} installed\n",
            if count_str == 1 { "" } else { "s" }
        ));
    } else if failed || notices.iter().any(|n| n.starts_with("[!] Unable")) {
        out.push_str("\x1b[31m✗\x1b[0m Pod install failed\n");
    }

    // Notices ([!] lines)
    if !notices.is_empty() {
        out.push('\n');
        for n in &notices {
            out.push_str(&format!("\x1b[33m{n}\x1b[0m\n"));
        }
    }

    // Installed pods
    if !installed_pods.is_empty() {
        out.push('\n');
        for pod in &installed_pods {
            out.push_str(&format!("  {pod}\n"));
        }
    }

    // "Using" pods — only in verbose mode
    if verbosity == Verbosity::Verbose && !using_pods.is_empty() {
        if installed_pods.is_empty() {
            out.push('\n');
        }
        for pod in &using_pods {
            out.push_str(&format!("  (unchanged) {pod}\n"));
        }
    }

    // If nothing was collected, passthrough
    if out.is_empty() {
        return FilterOutput::passthrough(raw);
    }

    let filtered_bytes = out.len();
    FilterOutput {
        content: out,
        original_bytes,
        filtered_bytes,
        structured: None,
    }
}

/// Parse `PodName (version) [source]` → `"PodName version"`.
fn parse_pod_line(rest: &str) -> Option<String> {
    let rest = rest.trim();
    // Format: "Alamofire (5.8.1)" or "Alamofire (5.8.1) [some source]"
    if let Some(paren_start) = rest.find('(') {
        let name = rest[..paren_start].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let after = &rest[paren_start + 1..];
        let version = after.split_once(')').map(|(v, _)| v.trim()).unwrap_or("?");
        Some(format!("{name} {version}"))
    } else {
        // No version info, just the name
        let name = rest.trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}

/// Extract pod count from completion line.
fn extract_pod_count(line: &str) -> Option<usize> {
    // "Pod installation complete! There are 3 dependencies from the Podfile and 3 total pods installed."
    // Look for "N total pods installed" or similar
    let words: Vec<&str> = line.split_whitespace().collect();
    for (i, w) in words.iter().enumerate() {
        if *w == "total" && i > 0 {
            if let Ok(n) = words[i - 1].parse::<usize>() {
                return Some(n);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_INSTALL: &str = "\
Analyzing dependencies
Downloading dependencies
Installing Alamofire (5.8.1)
Installing AlamofireImage (4.3.0)
Installing KeychainAccess (4.2.2)
Generating Pods project
Integrating client project
Pod installation complete! There are 3 dependencies from the Podfile and 3 total pods installed.
";

    const SAMPLE_WITH_WARNINGS: &str = "\
Analyzing dependencies
[!] The `Podfile` requires CocoaPods 1.11.0, but you're using 1.10.1.
Downloading dependencies
Installing Alamofire (5.8.1)
Installing RxSwift (6.6.0)
[!] Unable to find a specification for `SomeOldPod (~> 2.0)`
Generating Pods project
";

    const SAMPLE_UPDATE: &str = "\
Analyzing dependencies
Fetching podspec for `Alamofire` from `../Alamofire`
Downloading dependencies
Using Alamofire (5.8.1)
Using AlamofireImage (4.3.0)
Generating Pods project
Integrating client project
Pod installation complete! There are 3 dependencies from the Podfile and 3 total pods installed.
";

    #[test]
    fn compact_install_shows_result() {
        let out = filter(SAMPLE_INSTALL, Verbosity::Compact);
        assert!(out.content.contains("✓"));
        assert!(out.content.contains("3 pods installed"));
    }

    #[test]
    fn compact_install_lists_pods() {
        let out = filter(SAMPLE_INSTALL, Verbosity::Compact);
        assert!(out.content.contains("Alamofire 5.8.1"));
        assert!(out.content.contains("KeychainAccess 4.2.2"));
    }

    #[test]
    fn compact_strips_noise_lines() {
        let out = filter(SAMPLE_INSTALL, Verbosity::Compact);
        assert!(!out.content.contains("Analyzing dependencies"));
        assert!(!out.content.contains("Generating Pods project"));
    }

    #[test]
    fn compact_shows_warnings() {
        let out = filter(SAMPLE_WITH_WARNINGS, Verbosity::Compact);
        assert!(out.content.contains("CocoaPods 1.11.0"));
        assert!(out.content.contains("SomeOldPod"));
    }

    #[test]
    fn compact_update_hides_using_pods() {
        let out = filter(SAMPLE_UPDATE, Verbosity::Compact);
        assert!(!out.content.contains("(unchanged)"));
        assert!(!out.content.contains("Alamofire 5.8.1"));
    }

    #[test]
    fn verbose_update_shows_using_pods() {
        let out = filter(SAMPLE_UPDATE, Verbosity::Verbose);
        assert!(out.content.contains("(unchanged)"));
        assert!(out.content.contains("Alamofire 5.8.1"));
    }

    #[test]
    fn very_verbose_returns_passthrough() {
        let out = filter(SAMPLE_INSTALL, Verbosity::VeryVerbose);
        assert_eq!(out.content, SAMPLE_INSTALL);
    }

    #[test]
    fn bytes_reduced_vs_original() {
        let out = filter(SAMPLE_INSTALL, Verbosity::Compact);
        assert!(out.filtered_bytes < out.original_bytes);
    }
}
