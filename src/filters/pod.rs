use crate::filters::types::PodResult;
use crate::filters::{FilterOutput, Verbosity};

pub fn parse(raw: &str) -> PodResult {
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
        if trimmed.starts_with("[!]") {
            notices.push(trimmed.to_string());
            continue;
        }
        if trimmed.starts_with("Pod installation complete") || trimmed.contains("pods installed") {
            completion_line = Some(trimmed.to_string());
            continue;
        }
        if trimmed.starts_with("Error:")
            || trimmed.starts_with("error:")
            || trimmed.starts_with("[!] Unable to")
        {
            failed = true;
            notices.push(trimmed.to_string());
            continue;
        }
        if noise.iter().any(|n| trimmed.starts_with(n)) {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Installing ") {
            if let Some(pod) = parse_pod_line(rest) {
                installed_pods.push(pod);
            }
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("Using ") {
            if let Some(pod) = parse_pod_line(rest) {
                using_pods.push(pod);
            }
        }
    }

    let succeeded = completion_line.is_some() && !failed;
    let total_pods = if let Some(ref c) = completion_line {
        extract_pod_count(c).unwrap_or(installed_pods.len() + using_pods.len())
    } else {
        installed_pods.len() + using_pods.len()
    };

    PodResult {
        succeeded,
        installed_pods,
        using_pods,
        notices,
        total_pods,
    }
}

/// Filter `pod install` / `pod update` output.
///
/// Compact:
/// - Result line: `✓ Pod install complete — N pods installed` or `✗ Pod install failed`
/// - List installed pods: `  PodName version`
/// - Show `[!]` warning/error lines always
/// - Strip noise lines
///
/// Verbose: same + show "Using" (unchanged) pods too.
/// VeryVerbose+: raw passthrough.
pub fn filter(raw: &str, verbosity: Verbosity) -> FilterOutput {
    let original_bytes = raw.len();

    if matches!(verbosity, Verbosity::VeryVerbose | Verbosity::Maximum) {
        return FilterOutput::passthrough(raw);
    }

    let result = parse(raw);

    let mut out = String::new();

    if result.succeeded {
        let action = if result.using_pods.is_empty() {
            "install"
        } else {
            "update"
        };
        out.push_str(&format!(
            "\x1b[32m✓\x1b[0m Pod {action} complete — {} pod{} installed\n",
            result.total_pods,
            if result.total_pods == 1 { "" } else { "s" }
        ));
    } else if !result.succeeded
        && result.notices.iter().any(|n| {
            n.starts_with("[!] Unable") || n.starts_with("Error:") || n.starts_with("error:")
        })
    {
        out.push_str("\x1b[31m✗\x1b[0m Pod install failed\n");
    }

    if !result.notices.is_empty() {
        out.push('\n');
        for n in &result.notices {
            out.push_str(&format!("\x1b[33m{n}\x1b[0m\n"));
        }
    }

    if !result.installed_pods.is_empty() {
        out.push('\n');
        for pod in &result.installed_pods {
            out.push_str(&format!("  {pod}\n"));
        }
    }

    if verbosity == Verbosity::Verbose && !result.using_pods.is_empty() {
        if result.installed_pods.is_empty() {
            out.push('\n');
        }
        for pod in &result.using_pods {
            out.push_str(&format!("  (unchanged) {pod}\n"));
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
        structured: serde_json::to_value(&result).ok(),
    }
}

fn parse_pod_line(rest: &str) -> Option<String> {
    let rest = rest.trim();
    if let Some(paren_start) = rest.find('(') {
        let name = rest[..paren_start].trim().to_string();
        if name.is_empty() {
            return None;
        }
        let after = &rest[paren_start + 1..];
        let version = after.split_once(')').map(|(v, _)| v.trim()).unwrap_or("?");
        Some(format!("{name} {version}"))
    } else {
        let name = rest.trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }
}

fn extract_pod_count(line: &str) -> Option<usize> {
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

    #[test]
    fn parse_returns_structured_data() {
        let result = parse(SAMPLE_INSTALL);
        assert!(result.succeeded);
        assert_eq!(result.installed_pods.len(), 3);
        assert_eq!(result.total_pods, 3);
    }

    #[test]
    fn structured_is_some_on_filter() {
        let out = filter(SAMPLE_INSTALL, Verbosity::Compact);
        assert!(out.structured.is_some());
    }
}
