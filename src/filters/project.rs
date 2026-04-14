use std::path::Path;

use crate::filters::types::{ProjectDependency, ProjectResult, ProjectSourceCounts, ProjectTarget};
use crate::filters::{FilterOutput, Verbosity};

/// Analyse an iOS/macOS project directory and return a compact snapshot.
///
/// Reads (if present):
///   - `Podfile.lock`         → CocoaPods dependencies
///   - `Package.resolved`     → SPM dependencies
///   - `Cartfile.resolved`    → Carthage dependencies
///   - `*.xcodeproj/project.pbxproj` → targets (basic extraction)
///
/// Compact output: targets, min iOS, dependencies, source file counts.
pub fn filter_project(project_path: &str, verbosity: Verbosity) -> FilterOutput {
    let result = analyse(project_path);
    let content = render(&result, verbosity);
    let original_bytes = content.len(); // no raw command output here
    let filtered_bytes = content.len();
    let structured = serde_json::to_value(&result).ok();
    FilterOutput {
        content,
        original_bytes,
        filtered_bytes,
        structured,
    }
}

pub fn analyse(project_path: &str) -> ProjectResult {
    let root = Path::new(project_path);

    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown")
        .trim_end_matches(".xcodeproj")
        .trim_end_matches(".xcworkspace")
        .to_string();

    let mut result = ProjectResult {
        name,
        ..ProjectResult::default()
    };

    // Resolve actual project root if given a .xcodeproj or .xcworkspace
    let search_root = if root
        .extension()
        .is_some_and(|e| e == "xcodeproj" || e == "xcworkspace")
    {
        root.parent().unwrap_or(root)
    } else {
        root
    };

    // Read dependencies
    result.dependencies.extend(read_podfile_lock(search_root));
    result
        .dependencies
        .extend(read_package_resolved(search_root));
    result
        .dependencies
        .extend(read_cartfile_resolved(search_root));

    // Read project targets from pbxproj
    let (targets, min_ios, configs) = read_pbxproj_basic(search_root);
    result.targets = targets;
    result.min_ios = min_ios;
    result.configurations = configs;

    // Count source files
    result.source_counts = count_source_files(search_root);

    result
}

// ---------------------------------------------------------------------------
// Podfile.lock
// ---------------------------------------------------------------------------

fn read_podfile_lock(root: &Path) -> Vec<ProjectDependency> {
    let lock_path = root.join("Podfile.lock");
    let content = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::new();
    let mut in_pods = false;

    for line in content.lines() {
        if line.starts_with("PODS:") {
            in_pods = true;
            continue;
        }
        if in_pods && !line.starts_with(' ') && !line.is_empty() {
            break; // End of PODS section
        }
        if in_pods {
            // Lines like: "  - Alamofire (5.8.1):" or "  - Alamofire/Core (5.8.1)"
            let trimmed = line.trim_start_matches(' ').trim_start_matches('-').trim();
            if trimmed.is_empty() || trimmed.starts_with('-') {
                continue;
            }
            // Only top-level pods (2 spaces indent = top-level)
            if !line.starts_with("  - ") || line.starts_with("    ") {
                continue;
            }

            let dep = parse_pod_line(trimmed);
            // Deduplicate by name (avoid subspecs)
            if !deps.iter().any(|d: &ProjectDependency| d.name == dep.name) {
                deps.push(dep);
            }
        }
    }

    deps
}

fn parse_pod_line(line: &str) -> ProjectDependency {
    // "Firebase/Analytics (10.15.0):" → name="Firebase/Analytics", version="10.15.0"
    // "Alamofire (5.8.1)" → name="Alamofire", version="5.8.1"
    let line = line.trim_end_matches(':');
    if let Some(paren_start) = line.rfind('(') {
        let name = line[..paren_start].trim().to_string();
        let version = line[paren_start + 1..].trim_end_matches(')').to_string();
        ProjectDependency {
            name,
            version,
            manager: "CocoaPods".to_string(),
        }
    } else {
        ProjectDependency {
            name: line.to_string(),
            version: String::new(),
            manager: "CocoaPods".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Package.resolved (SPM)
// ---------------------------------------------------------------------------

fn read_package_resolved(root: &Path) -> Vec<ProjectDependency> {
    // Can be at root or inside .xcodeproj
    let candidates = [
        root.join("Package.resolved"),
        root.join("*.xcodeproj")
            .join("project.xcworkspace")
            .join("xcshareddata")
            .join("swiftpm")
            .join("Package.resolved"),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            return parse_package_resolved(&content);
        }
    }

    // Walk for Package.resolved inside any .xcodeproj
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "xcodeproj") {
                let resolved = p
                    .join("project.xcworkspace")
                    .join("xcshareddata")
                    .join("swiftpm")
                    .join("Package.resolved");
                if let Ok(content) = std::fs::read_to_string(&resolved) {
                    return parse_package_resolved(&content);
                }
            }
        }
    }

    Vec::new()
}

fn parse_package_resolved(content: &str) -> Vec<ProjectDependency> {
    let mut deps = Vec::new();

    // Simple line-by-line extraction — works for both v1 and v2 format
    let mut current_name = String::new();
    let mut current_version = String::new();

    for line in content.lines() {
        let t = line.trim();

        // v2: "identity" : "alamofire",
        // v1: "package" : "Alamofire",
        if t.contains("\"identity\"") || t.contains("\"package\"") {
            current_name = extract_json_value(t);
        }

        if t.contains("\"version\"") {
            current_version = extract_json_value(t);
        }

        // When we have both, emit
        if !current_name.is_empty() && !current_version.is_empty() {
            deps.push(ProjectDependency {
                name: pascal_case_name(&current_name),
                version: current_version.clone(),
                manager: "SPM".to_string(),
            });
            current_name.clear();
            current_version.clear();
        }
    }

    deps
}

fn extract_json_value(line: &str) -> String {
    // Extract the string value from `"key" : "value",`
    let parts: Vec<&str> = line.splitn(2, ':').collect();
    if parts.len() < 2 {
        return String::new();
    }
    parts[1]
        .trim()
        .trim_matches(',')
        .trim()
        .trim_matches('"')
        .to_string()
}

fn pascal_case_name(s: &str) -> String {
    // "alamofire" → "Alamofire", "swift-collections" → "SwiftCollections"
    s.split(['-', '_'])
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Cartfile.resolved (Carthage)
// ---------------------------------------------------------------------------

fn read_cartfile_resolved(root: &Path) -> Vec<ProjectDependency> {
    let path = root.join("Cartfile.resolved");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::new();

    for line in content.lines() {
        // Format: `github "Alamofire/Alamofire" "5.8.1"`
        //         `git "https://github.com/org/repo" "1.0.0"`
        let parts: Vec<&str> = line.split('"').collect();
        if parts.len() >= 4 {
            let repo = parts[1]; // "Alamofire/Alamofire" or URL
            let version = parts[3]; // "5.8.1"
            let name = repo.split('/').next_back().unwrap_or(repo).to_string();
            deps.push(ProjectDependency {
                name,
                version: version.to_string(),
                manager: "Carthage".to_string(),
            });
        }
    }

    deps
}

// ---------------------------------------------------------------------------
// Basic pbxproj parsing (targets + configurations)
// ---------------------------------------------------------------------------

fn read_pbxproj_basic(root: &Path) -> (Vec<ProjectTarget>, String, Vec<String>) {
    // Find first .xcodeproj
    let xcodeproj = match find_xcodeproj(root) {
        Some(p) => p,
        None => return (Vec::new(), String::new(), Vec::new()),
    };

    let pbxproj = xcodeproj.join("project.pbxproj");
    let content = match std::fs::read_to_string(&pbxproj) {
        Ok(c) => c,
        Err(_) => return (Vec::new(), String::new(), Vec::new()),
    };

    let targets = extract_targets_basic(&content);
    let min_ios = extract_min_ios(&content);
    let configs = extract_configurations(&content);

    (targets, min_ios, configs)
}

fn find_xcodeproj(root: &Path) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "xcodeproj") {
                return Some(p);
            }
        }
    }
    None
}

fn extract_targets_basic(content: &str) -> Vec<ProjectTarget> {
    let mut targets = Vec::new();

    // Look for lines like: `name = MyApp;` inside PBXNativeTarget sections
    // This is a heuristic — not a full pbxproj parser
    let mut in_native_target = false;
    let mut current_name = String::new();
    let mut current_type = String::new();

    for line in content.lines() {
        let t = line.trim();

        if t.contains("isa = PBXNativeTarget") {
            in_native_target = true;
            current_name.clear();
            current_type.clear();
            continue;
        }

        if in_native_target {
            if t == "};" || t == "}" {
                if !current_name.is_empty() {
                    targets.push(ProjectTarget {
                        name: current_name.clone(),
                        bundle_id: String::new(), // filled later if needed
                        kind: normalize_target_type(&current_type),
                    });
                }
                in_native_target = false;
                continue;
            }

            if t.starts_with("name = ") {
                current_name = t
                    .trim_start_matches("name = ")
                    .trim_end_matches(';')
                    .trim_matches('"')
                    .to_string();
            }

            if t.starts_with("productType = ") {
                current_type = t
                    .trim_start_matches("productType = ")
                    .trim_end_matches(';')
                    .trim_matches('"')
                    .to_string();
            }
        }
    }

    targets
}

fn normalize_target_type(product_type: &str) -> String {
    match product_type {
        s if s.contains("application") => "App".to_string(),
        s if s.contains("unit-test") => "Unit Tests".to_string(),
        s if s.contains("ui-test") => "UI Tests".to_string(),
        s if s.contains("extension") => "Extension".to_string(),
        s if s.contains("framework") => "Framework".to_string(),
        s if s.contains("static-library") => "Static Library".to_string(),
        s if s.contains("dynamic-library") => "Dynamic Library".to_string(),
        s if s.contains("watch") => "watchOS App".to_string(),
        s if s.contains("widget") => "Widget".to_string(),
        _ => product_type
            .split('.')
            .next_back()
            .unwrap_or(product_type)
            .to_string(),
    }
}

fn extract_min_ios(content: &str) -> String {
    for line in content.lines() {
        let t = line.trim();
        if t.starts_with("IPHONEOS_DEPLOYMENT_TARGET = ") {
            return t
                .trim_start_matches("IPHONEOS_DEPLOYMENT_TARGET = ")
                .trim_end_matches(';')
                .to_string();
        }
    }
    String::new()
}

fn extract_configurations(content: &str) -> Vec<String> {
    let mut configs = std::collections::BTreeSet::new();
    for line in content.lines() {
        let t = line.trim();
        // Lines like: `Debug /* Debug */ = {`  or `name = Release;`
        if t.starts_with("name = ") && t.ends_with(';') {
            let name = t
                .trim_start_matches("name = ")
                .trim_end_matches(';')
                .trim_matches('"')
                .to_string();
            // Filter out non-config names (targets, files etc)
            if is_build_config(&name) {
                configs.insert(name);
            }
        }
    }
    configs.into_iter().collect()
}

fn is_build_config(name: &str) -> bool {
    matches!(
        name,
        "Debug"
            | "Release"
            | "Staging"
            | "Beta"
            | "Profile"
            | "Distribution"
            | "AdHoc"
            | "AppStore"
    ) || name.contains("Debug")
        || name.contains("Release")
        || name.contains("Staging")
}

// ---------------------------------------------------------------------------
// Source file counting
// ---------------------------------------------------------------------------

fn count_source_files(root: &Path) -> ProjectSourceCounts {
    let mut counts = ProjectSourceCounts::default();

    count_recursive(root, &mut counts, 0);

    counts
}

fn count_recursive(dir: &Path, counts: &mut ProjectSourceCounts, depth: u32) {
    if depth > 8 {
        return;
    }

    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Skip noisy directories
    const SKIP_DIRS: &[&str] = &[
        "DerivedData",
        ".build",
        "Pods",
        ".git",
        "xcuserdata",
        "xcshareddata",
        "node_modules",
        ".swiftpm",
    ];
    if SKIP_DIRS.contains(&dir_name) {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            count_recursive(&path, counts, depth + 1);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext {
                "swift" => counts.swift += 1,
                "m" | "mm" => counts.objc += 1,
                "storyboard" => counts.storyboards += 1,
                "xib" => counts.xibs += 1,
                "png" | "jpg" | "jpeg" | "pdf" | "xcassets" => counts.resources += 1,
                _ => {}
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(result: &ProjectResult, verbosity: Verbosity) -> String {
    let mut out = String::new();

    out.push_str(&format!("Project: {}\n", result.name));

    // Targets
    if !result.targets.is_empty() {
        let target_names: Vec<String> = result
            .targets
            .iter()
            .map(|t| {
                if t.kind == "App" {
                    t.name.clone()
                } else {
                    format!("{} ({})", t.name, t.kind)
                }
            })
            .collect();
        out.push_str(&format!("Targets: {}\n", target_names.join(", ")));
    }

    // Versions
    let mut meta = Vec::new();
    if !result.min_ios.is_empty() {
        meta.push(format!("Min iOS {}", result.min_ios));
    }
    if !result.configurations.is_empty() {
        meta.push(format!("Configs: {}", result.configurations.join(", ")));
    }
    if !meta.is_empty() {
        out.push_str(&format!("{}\n", meta.join("  |  ")));
    }

    // Dependencies
    if !result.dependencies.is_empty() {
        out.push('\n');

        // Group by manager
        let mut by_manager: std::collections::BTreeMap<String, Vec<&ProjectDependency>> =
            std::collections::BTreeMap::new();
        for dep in &result.dependencies {
            by_manager.entry(dep.manager.clone()).or_default().push(dep);
        }

        for (manager, deps) in &by_manager {
            out.push_str(&format!(
                "Dependencies ({} — {} packages):\n",
                manager,
                deps.len()
            ));
            let limit = if matches!(verbosity, Verbosity::Compact) {
                10
            } else {
                deps.len()
            };
            for dep in deps.iter().take(limit) {
                out.push_str(&format!("  {} {}\n", dep.name, dep.version));
            }
            let hidden = deps.len().saturating_sub(limit);
            if hidden > 0 {
                out.push_str(&format!("  [+{} more — use -v to list all]\n", hidden));
            }
        }
    }

    // Source counts
    let counts = &result.source_counts;
    if counts.swift + counts.objc + counts.storyboards + counts.xibs > 0 {
        out.push('\n');
        let mut parts = Vec::new();
        if counts.swift > 0 {
            parts.push(format!("{} Swift", counts.swift));
        }
        if counts.objc > 0 {
            parts.push(format!("{} ObjC", counts.objc));
        }
        if counts.storyboards > 0 {
            parts.push(format!("{} storyboards", counts.storyboards));
        }
        if counts.xibs > 0 {
            parts.push(format!("{} xibs", counts.xibs));
        }
        out.push_str(&format!("Sources: {}\n", parts.join(", ")));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pod_line_with_version() {
        let dep = parse_pod_line("Firebase/Analytics (10.15.0):");
        assert_eq!(dep.name, "Firebase/Analytics");
        assert_eq!(dep.version, "10.15.0");
        assert_eq!(dep.manager, "CocoaPods");
    }

    #[test]
    fn parse_pod_line_simple() {
        let dep = parse_pod_line("Alamofire (5.8.1)");
        assert_eq!(dep.name, "Alamofire");
        assert_eq!(dep.version, "5.8.1");
    }

    #[test]
    fn pascal_case_converts_kebab() {
        assert_eq!(pascal_case_name("swift-collections"), "SwiftCollections");
        assert_eq!(pascal_case_name("alamofire"), "Alamofire");
    }

    #[test]
    fn target_type_normalization() {
        assert_eq!(
            normalize_target_type("com.apple.product-type.application"),
            "App"
        );
        assert_eq!(
            normalize_target_type("com.apple.product-type.bundle.unit-test"),
            "Unit Tests"
        );
        assert_eq!(
            normalize_target_type("com.apple.product-type.app-extension"),
            "Extension"
        );
    }

    const SAMPLE_PODFILE_LOCK: &str = r#"PODS:
  - Alamofire (5.8.1)
  - Firebase/Analytics (10.15.0):
    - Firebase/Core
  - Firebase/Core (10.15.0):
    - FirebaseCore
  - FirebaseCore (10.15.0)
  - Kingfisher (7.10.2)

DEPENDENCIES:
  - Alamofire (~> 5.8)
"#;

    #[test]
    fn reads_podfile_lock_top_level_only() {
        let deps = parse_podfile_lock_str(SAMPLE_PODFILE_LOCK);
        // Should get Alamofire, Firebase/Analytics, Firebase/Core, FirebaseCore, Kingfisher
        // but NOT Firebase/Core as subspec of Analytics (it's also top-level so it counts)
        assert!(deps.iter().any(|d| d.name == "Alamofire"));
        assert!(deps.iter().any(|d| d.name == "Kingfisher"));
        // No duplicates
        let names: Vec<_> = deps.iter().map(|d| d.name.as_str()).collect();
        let unique: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(names.len(), unique.len());
    }

    fn parse_podfile_lock_str(content: &str) -> Vec<ProjectDependency> {
        let mut deps = Vec::new();
        let mut in_pods = false;
        for line in content.lines() {
            if line.starts_with("PODS:") {
                in_pods = true;
                continue;
            }
            if in_pods && !line.starts_with(' ') && !line.is_empty() {
                break;
            }
            if in_pods {
                let trimmed = line.trim_start_matches(' ').trim_start_matches('-').trim();
                if trimmed.is_empty() {
                    continue;
                }
                if !line.starts_with("  - ") || line.starts_with("    ") {
                    continue;
                }
                let dep = parse_pod_line(trimmed);
                if !deps.iter().any(|d: &ProjectDependency| d.name == dep.name) {
                    deps.push(dep);
                }
            }
        }
        deps
    }
}
